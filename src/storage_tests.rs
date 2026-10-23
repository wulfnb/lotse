use super::*;
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "wega-storage-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn lock_excludes_other_instances_and_is_released() {
    let temp = Temp::new();
    let first = FileProfileStore::open(&temp.0).unwrap();
    assert!(FileProfileStore::open(&temp.0).is_err());
    drop(first);
    assert!(FileProfileStore::open(&temp.0).is_ok());
}
#[test]
fn corrupt_profile_cannot_be_overwritten() {
    let temp = Temp::new();
    fs::write(temp.0.join("browser.json"), b"broken").unwrap();
    let mut store = FileProfileStore::open(&temp.0).unwrap();
    assert!(store.save(&Profile::default()).is_err());
    assert!(store.load().is_err());
    assert!(store.save(&Profile::default()).is_err());
    assert_eq!(fs::read(temp.0.join("browser.json")).unwrap(), b"broken");
}
#[test]
fn interrupted_file_does_not_replace_committed_profile() {
    let temp = Temp::new();
    let mut store = FileProfileStore::open(&temp.0).unwrap();
    let mut profile = store.load().unwrap();
    profile.urls = vec!["https://example.com/".into()];
    store.save(&profile).unwrap();
    fs::write(temp.0.join("browser.json.tmp"), b"interrupted").unwrap();
    fs::write(
        temp.0
            .join(format!(".browser-{}-0.tmp", std::process::id())),
        b"stale",
    )
    .unwrap();
    assert_eq!(store.load().unwrap().urls, profile.urls);
    store.save(&profile).unwrap();
    assert_eq!(store.load().unwrap().urls, profile.urls);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(temp.0.join("browser.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
#[test]
fn failed_write_preserves_committed_bytes_and_cleans_temp() {
    let temp = Temp::new();
    let destination = temp.0.join("browser.json");
    fs::create_dir(&destination).unwrap(); // deterministic rename failure, including under root
    fs::write(destination.join("sentinel"), b"keep").unwrap();
    assert!(atomic_write(&destination, b"new").is_err());
    assert_eq!(fs::read(destination.join("sentinel")).unwrap(), b"keep");
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
}

#[test]
fn filesystem_errors_do_not_create_or_overwrite_a_profile() {
    let temp = Temp::new();
    let blocker = temp.0.join("not-a-directory");
    fs::write(&blocker, b"keep").unwrap();
    assert!(FileProfileStore::open(&blocker).is_err());
    fs::create_dir(temp.0.join("browser.lock")).unwrap();
    assert!(FileProfileStore::open(&temp.0).is_err());
    fs::create_dir(temp.0.join("browser.json")).unwrap();
    assert!(Profile::load(&temp.0).is_err());
    assert!(Profile::default().save(&blocker).is_err());
    assert_eq!(fs::read(blocker).unwrap(), b"keep");
}

#[test]
fn bounded_temporary_name_collisions_fail_without_overwriting_them() {
    let temp = Temp::new();
    for counter in 0..=1024 {
        fs::write(
            temp.0
                .join(format!(".browser-{}-{counter}.tmp", std::process::id())),
            b"stale",
        )
        .unwrap();
    }
    assert_eq!(
        atomic_write(&temp.0.join("browser.json"), b"new")
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(!temp.0.join("browser.json").exists());
    assert_eq!(
        fs::read(
            temp.0
                .join(format!(".browser-{}-1024.tmp", std::process::id()))
        )
        .unwrap(),
        b"stale"
    );
}

#[test]
fn a_root_path_without_a_parent_is_rejected_before_any_io() {
    assert!(atomic_write(Path::new("/"), b"must not write").is_err());
}
