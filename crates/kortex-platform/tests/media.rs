#![cfg(not(target_arch = "wasm32"))]
use kortex_platform::media::{Broker, Error, Limits, MAX_READ_BYTES};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture {
    root: PathBuf,
    file: PathBuf,
}
impl Fixture {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "kortex-media-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).expect("fixture directory");
        let file = root.join("selected.bin");
        std::fs::write(&file, bytes).expect("fixture bytes");
        Self { root, file }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.file);
        let _ = std::fs::remove_dir(&self.root);
    }
}
fn broker() -> Broker {
    Broker::new(Limits::default()).expect("limits")
}

#[test]
fn reads_only_requested_ranges_and_metadata_has_no_path() {
    let f = Fixture::new(b"0123456789");
    let mut b = broker();
    let h = b
        .register_native("p", &f.file, "video/mp4")
        .expect("selected file");
    let meta = b.metadata("p", &h).expect("metadata");
    assert_eq!(meta.size_bytes, 10);
    assert_eq!(meta.mime, "video/mp4");
    assert_eq!(format!("{h:?}"), "Handle(<opaque>)");
    assert_eq!(b.read("p", &h, 3, 4), Ok(b"3456".to_vec()));
    assert_eq!(b.read("p", &h, 10, 0), Ok(Vec::new()));
    assert_eq!(b.read("p", &h, 0, 10), Ok(b"0123456789".to_vec()));
}
#[test]
fn ranges_and_limits_fail_before_allocation_or_reading() {
    let f = Fixture::new(b"0123456789");
    let mut b = Broker::new(Limits {
        max_handles: 1,
        max_read_bytes: 4,
    })
    .expect("limits");
    let h = b.register_native("p", &f.file, "").expect("selected file");
    assert_eq!(b.read("p", &h, 0, usize::MAX), Err(Error::ReadLimit));
    assert_eq!(b.read("p", &h, u64::MAX, 2), Err(Error::Bounds));
    assert_eq!(b.read("p", &h, 9, 2), Err(Error::Bounds));
    assert_eq!(b.read("p", &h, 11, 0), Err(Error::Bounds));
    assert_eq!(
        b.register_native("p", &f.file, "").err(),
        Some(Error::HandleQuota)
    );
    assert_eq!(b.read("p", &h, 1, 3), Ok(b"123".to_vec()));
    assert!(
        Broker::new(Limits {
            max_handles: 0,
            max_read_bytes: 1
        })
        .is_err()
    );
    assert!(
        Broker::new(Limits {
            max_handles: 1,
            max_read_bytes: MAX_READ_BYTES + 1
        })
        .is_err()
    );
}
#[test]
fn capabilities_are_bound_to_project_broker_and_revoked_clones() {
    let f = Fixture::new(b"data");
    let mut a = Broker::new(Limits {
        max_handles: 1,
        max_read_bytes: 4,
    })
    .expect("limits");
    let mut b = broker();
    let ha = a.register_native("project-a", &f.file, "").expect("file");
    let hb = b
        .register_native("project-a", &f.file, "")
        .expect("same slot different broker");
    assert_eq!(b.read("project-a", &ha, 0, 1), Err(Error::InvalidHandle));
    assert_eq!(a.read("project-a", &hb, 0, 1), Err(Error::InvalidHandle));
    assert_eq!(a.read("project-b", &ha, 0, 1), Err(Error::WrongProject));
    assert_eq!(a.revoke("project-b", &ha), Err(Error::WrongProject));
    let cloned = ha.clone();
    a.revoke("project-a", &ha).expect("revoke");
    let replacement = a
        .register_native("project-a", &f.file, "")
        .expect("new capability");
    assert_eq!(
        a.read("project-a", &cloned, 0, 1),
        Err(Error::InvalidHandle)
    );
    assert_eq!(a.read("project-a", &replacement, 0, 1), Ok(b"d".to_vec()));
    a.revoke_project("project-a");
    assert_eq!(
        a.metadata("project-a", &replacement).err(),
        Some(Error::InvalidHandle)
    );
}
#[test]
fn changed_media_invalidates_all_clones_without_returning_bytes() {
    let f = Fixture::new(b"data");
    let mut b = broker();
    let h = b.register_native("p", &f.file, "").expect("file");
    OpenOptions::new()
        .write(true)
        .open(&f.file)
        .expect("writer")
        .set_len(2)
        .expect("truncate");
    assert_eq!(b.read("p", &h, 0, 1), Err(Error::Changed));
    assert_eq!(b.read("p", &h.clone(), 0, 1), Err(Error::InvalidHandle));
}
#[test]
fn invalid_host_registration_does_not_consume_a_handle() {
    let f = Fixture::new(b"data");
    let mut b = Broker::new(Limits {
        max_handles: 1,
        max_read_bytes: 4,
    })
    .expect("limits");
    assert_eq!(
        b.register_native("", &f.file, "").err(),
        Some(Error::InvalidProject)
    );
    assert_eq!(
        b.register_native("p", &f.file, "bad\nmime").err(),
        Some(Error::InvalidMime)
    );
    assert_eq!(b.register_native("p", &f.root, "").err(), Some(Error::Io));
    assert!(b.register_native("p", &f.file, "").is_ok());
}
#[test]
fn reads_the_tail_of_a_40_gib_sparse_file_without_loading_it() {
    let f = Fixture::new(b"");
    // Mark sparse before extending on Windows; never reserve a 40 GiB ordinary file.
    #[cfg(windows)]
    {
        let output = std::process::Command::new("fsutil")
            .args(["sparse", "setflag"])
            .arg(&f.file)
            .output();
        if !output.is_ok_and(|o| o.status.success()) {
            eprintln!("Sparse fixture skipped: fsutil sparse setflag unavailable on this host");
            return;
        }
    }
    let size = 40_u64 * 1024 * 1024 * 1024;
    let mut file = File::options()
        .write(true)
        .open(&f.file)
        .expect("fixture writer");
    file.set_len(size).expect("sparse size");
    file.seek(SeekFrom::Start(size - 4)).expect("seek end");
    file.write_all(b"tail").expect("tail bytes");
    file.sync_all().expect("flush");
    let mut b = broker();
    let h = b
        .register_native("p", &f.file, "video/mp4")
        .expect("selected sparse file");
    assert_eq!(b.metadata("p", &h).expect("metadata").size_bytes, size);
    assert_eq!(b.read("p", &h, size - 4, 4), Ok(b"tail".to_vec()));
    assert_eq!(b.read("p", &h, size - 2, 4), Err(Error::Bounds));
}

#[test]
fn same_size_edits_with_changed_timestamp_revoke_the_capability() {
    let f = Fixture::new(b"data");
    let mut b = broker();
    let h = b.register_native("p", &f.file, "").expect("file");
    let mut writer = OpenOptions::new()
        .write(true)
        .open(&f.file)
        .expect("writer");
    writer.write_all(b"edit").expect("replace bytes");
    writer
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(86_400))
        .expect("changed mtime");
    assert_eq!(b.read("p", &h, 0, 4), Err(Error::Changed));
    assert_eq!(b.metadata("p", &h).err(), Some(Error::InvalidHandle));
}
