//! Filesystem adapter: exclusive profile ownership, guarded recovery and durable replacement.
use crate::profile::Profile;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Application port. Callers cannot save a profile whose load failed.
pub trait ProfileStore {
    fn load(&mut self) -> Result<Profile, String>;
    fn save(&self, profile: &Profile) -> Result<(), String>;
}
pub struct FileProfileStore {
    directory: PathBuf,
    _lock: File,
    writable: bool,
}
impl FileProfileStore {
    pub fn open(directory: &Path) -> Result<Self, String> {
        fs::create_dir_all(directory).map_err(|e| format!("Cannot create profile: {e}"))?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("browser.lock"))
            .map_err(|e| format!("Cannot open profile lock: {e}"))?;
        lock.try_lock().map_err(|e| {
            format!(
                "Cannot lock profile {}: {e}. Use a different --profile-dir for another instance.",
                directory.display()
            )
        })?;
        Ok(Self {
            directory: directory.into(),
            _lock: lock,
            writable: false,
        })
    }
}
impl ProfileStore for FileProfileStore {
    fn load(&mut self) -> Result<Profile, String> {
        self.writable = false;
        let profile = Profile::load(&self.directory)?;
        self.writable = true;
        Ok(profile)
    }
    fn save(&self, profile: &Profile) -> Result<(), String> {
        if !self.writable {
            return Err(
                "Profile has not loaded successfully; existing data will not be overwritten".into(),
            );
        }
        profile.save(&self.directory)
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing parent directory"))?;
    fs::create_dir_all(directory)?;
    // create_new avoids following a stale temporary-file symlink. Interrupted temp
    // files never supersede the committed browser.json on the next startup.
    let mut counter = 0u64;
    let (temporary, mut file) = loop {
        let temporary = directory.join(format!(".browser-{}-{counter}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temporary) {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists && counter < 1024 => {
                counter += 1
            }
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        File::open(directory)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
