//! Authenticated viewer-to-picker handoff over a private, local IPC directory.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Serialize, Deserialize)]
struct Manifest {
    id: String,
    expires: u64,
    multiple: bool,
    origin: String,
    extensions: Vec<String>,
}
#[derive(Deserialize)]
struct Reply {
    id: String,
    paths: Vec<PathBuf>,
}
pub struct Request {
    directory: PathBuf,
    uploads: PathBuf,
    manifest: Manifest,
}
impl Request {
    pub fn new(
        directory: &Path,
        uploads: &Path,
        multiple: bool,
        origin: String,
        extensions: Vec<String>,
    ) -> Result<Self, String> {
        let mut random = [0u8; 16];
        fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut random))
            .map_err(|e| e.to_string())?;
        let id = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let manifest = Manifest {
            id,
            expires: now() + 300,
            multiple,
            origin,
            extensions,
        };
        let temporary = directory.join(format!(".current-{}", manifest.id));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o640);
        }
        let result = (|| -> Result<(), std::io::Error> {
            let mut file = options.open(&temporary)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(fs::Permissions::from_mode(0o640))?;
            }
            file.write_all(&serde_json::to_vec(&manifest)?)?;
            file.sync_all()?;
            fs::rename(&temporary, directory.join("current.json"))
        })();
        let _ = fs::remove_file(temporary);
        result.map_err(|e| e.to_string())?;
        Ok(Self {
            directory: directory.into(),
            uploads: uploads.into(),
            manifest,
        })
    }
    pub fn poll(&self) -> Result<Option<Vec<PathBuf>>, String> {
        if now() > self.manifest.expires {
            return Err(
                "The viewer file request expired. Use path entry or reopen the file chooser."
                    .into(),
            );
        }
        let path = self
            .directory
            .join(format!("reply-{}.json", self.manifest.id));
        let file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(65537)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let _ = fs::remove_file(path);
        let reply: Reply = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if reply.id != self.manifest.id
            || reply.paths.len() > 32
            || (!self.manifest.multiple && reply.paths.len() > 1)
        {
            return Err("Invalid viewer file selection.".into());
        }
        let root = self.uploads.canonicalize().map_err(|e| e.to_string())?;
        let paths = reply
            .paths
            .into_iter()
            .map(|path| {
                let path = path.canonicalize().map_err(|e| e.to_string())?;
                if !path.starts_with(&root) || !path.is_file() {
                    return Err("The selected file is outside the upload area.".into());
                }
                Ok(path)
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Some(paths))
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
impl Drop for Request {
    fn drop(&mut self) {
        let current = self.directory.join("current.json");
        if fs::read(&current)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Manifest>(&bytes).ok())
            .is_some_and(|value| value.id == self.manifest.id)
        {
            let _ = fs::remove_file(current);
        }
        let _ = fs::remove_file(
            self.directory
                .join(format!("reply-{}.json", self.manifest.id)),
        );
        let _ = fs::remove_file(
            self.directory
                .join(format!("accepted-{}", self.manifest.id)),
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_current_request_and_uploaded_regular_files_can_be_selected() {
        let directory =
            std::env::temp_dir().join(format!("wega-handoff-{}-{}", std::process::id(), now()));
        fs::create_dir_all(&directory).unwrap();
        let uploads = directory.join("uploads");
        fs::create_dir(&uploads).unwrap();
        let file = uploads.join("sample.txt");
        fs::write(&file, b"hello").unwrap();
        let first = Request::new(
            &directory,
            &uploads,
            false,
            "https://example.test".into(),
            vec![],
        )
        .unwrap();
        assert!(first.poll().unwrap().is_none());
        let second = Request::new(
            &directory,
            &uploads,
            false,
            "https://example.test".into(),
            vec![],
        )
        .unwrap();
        drop(first);
        assert!(directory.join("current.json").exists());
        let reply = directory.join(format!("reply-{}.json", second.manifest.id));
        fs::write(
            &reply,
            serde_json::json!({"id":second.manifest.id,"paths":[file]}).to_string(),
        )
        .unwrap();
        assert_eq!(second.poll().unwrap().unwrap().len(), 1);
        assert!(second.poll().unwrap().is_none());
        fs::write(
            &reply,
            serde_json::json!({"id":"stale","paths":[]}).to_string(),
        )
        .unwrap();
        assert!(second.poll().is_err());
        fs::write(
            &reply,
            serde_json::json!({"id":second.manifest.id,"paths":[directory.join("current.json")]})
                .to_string(),
        )
        .unwrap();
        assert!(second.poll().is_err());
        drop(second);
        assert!(!directory.join("current.json").exists());
        fs::remove_dir_all(directory).unwrap();
    }
}
