use super::*;

fn path(text: &str) -> SyncPath {
    SyncPath::parse(text, &[]).unwrap()
}

fn disk() -> ShareDisk {
    ShareDisk::default()
}

fn names(scan: &LocalScan) -> Vec<&str> {
    let mut names: Vec<&str> = scan.files.iter().map(|file| file.path.as_str()).collect();
    names.sort_unstable();
    names
}

#[test]
fn scans_only_markdown_outside_excluded_and_hidden_folders() {
    let root = tempfile::tempdir().unwrap();
    for file in [
        "README.md",
        "docs/guide.md",
        "docs/image.png",
        "docs/notes.txt",
        "node_modules/pkg/readme.md",
        ".git/notes.md",
        ".hidden/a.md",
    ] {
        let target = root.path().join(file);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, "x").unwrap();
    }

    let scan = disk().scan(root.path()).unwrap();

    assert_eq!(names(&scan), ["README.md", "docs/guide.md"]);
    assert!(scan.ignored.is_empty() && !scan.truncated);
}

#[test]
fn an_oversized_file_is_ignored_and_reported() {
    let root = tempfile::tempdir().unwrap();
    let big = vec![b'a'; usize::try_from(MAX_FILE_BYTES).unwrap() + 1];
    std::fs::write(root.path().join("big.md"), big).unwrap();
    std::fs::write(root.path().join("small.md"), "x").unwrap();

    let scan = disk().scan(root.path()).unwrap();

    assert_eq!(names(&scan), ["small.md"]);
    assert_eq!(scan.ignored.len(), 1);
    assert_eq!(scan.ignored[0].path, "big.md");
    assert_eq!(
        scan.ignored[0].reason,
        app::collection_sync::IgnoredReason::TooLarge
    );
}

#[test]
fn writes_atomically_and_replaces_an_existing_file() {
    let root = tempfile::tempdir().unwrap();
    let disk = disk();

    let first = disk
        .write(root.path(), &path("docs/deep/a.md"), b"one")
        .unwrap();
    let second = disk
        .write(root.path(), &path("docs/deep/a.md"), b"second")
        .unwrap();

    assert_eq!(first.size, 3);
    assert_eq!(second.size, 6);
    assert_eq!(
        disk.read(root.path(), &path("docs/deep/a.md")).unwrap(),
        b"second"
    );
    let leftovers: Vec<_> = std::fs::read_dir(root.path().join("docs/deep"))
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name())
        .collect();
    assert_eq!(
        leftovers.len(),
        1,
        "pas de fichier temporaire : {leftovers:?}"
    );
}

#[test]
fn a_temporary_file_is_never_listed_as_a_collection_file() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(".daftr-sync-1-0.tmp"), "x").unwrap();

    assert!(disk().scan(root.path()).unwrap().files.is_empty());
}

#[test]
fn refuses_a_directory_or_a_missing_file_for_reading() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("dir.md")).unwrap();
    let disk = disk();

    assert!(disk.read(root.path(), &path("dir.md")).is_err());
    assert!(disk.read(root.path(), &path("missing.md")).is_err());
    assert!(!disk.exists(root.path(), &path("dir.md")));
    assert!(disk.write(root.path(), &path("dir.md"), b"x").is_err());
}

#[test]
fn refuses_to_read_a_file_over_the_limit() {
    let root = tempfile::tempdir().unwrap();
    let big = vec![b'a'; usize::try_from(MAX_FILE_BYTES).unwrap() + 1];
    std::fs::write(root.path().join("big.md"), big).unwrap();

    assert!(disk().read(root.path(), &path("big.md")).is_err());
}

#[test]
fn prepares_a_missing_directory() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("received").join("Docs");
    let disk = disk();

    assert!(!disk.is_directory(&target));
    let prepared = disk.prepare_directory(&target).unwrap();

    assert!(disk.is_directory(&prepared));
    assert!(disk.is_empty_directory(&prepared));
    std::fs::write(prepared.join("a.md"), "x").unwrap();
    assert!(!disk.is_empty_directory(&prepared));
}

#[cfg(unix)]
#[test]
fn never_follows_a_symbolic_link() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.md"), "secret").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.md"),
        root.path().join("file.md"),
    )
    .unwrap();
    let disk = disk();

    assert!(disk.scan(root.path()).unwrap().files.is_empty());
    assert!(disk.read(root.path(), &path("link/secret.md")).is_err());
    assert!(disk.read(root.path(), &path("file.md")).is_err());
    assert!(disk.write(root.path(), &path("link/new.md"), b"x").is_err());
    assert!(disk.write(root.path(), &path("file.md"), b"x").is_err());
    assert!(!outside.path().join("new.md").exists());
    assert_eq!(
        std::fs::read(outside.path().join("secret.md")).unwrap(),
        b"secret"
    );
}
