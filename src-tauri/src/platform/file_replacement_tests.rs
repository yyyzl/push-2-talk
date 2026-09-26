use super::replace_file;
use std::fs;

#[test]
fn prepared_file_creates_a_missing_target_with_unicode_and_spaces() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("新 词库.tmp");
    let target = directory.path().join("缓存 词库.txt");
    let content = "【编程】:[Rust,Swift]\n";
    fs::write(&source, content).unwrap();

    replace_file(&source, &target).unwrap();

    assert_eq!(fs::read_to_string(&target).unwrap(), content);
    assert!(!source.exists());
}

#[test]
fn prepared_file_replaces_existing_content_and_consumes_source() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("prepared.tmp");
    let target = directory.path().join("existing.txt");
    fs::write(&target, "old content that is longer than the replacement").unwrap();
    fs::write(&source, "new content").unwrap();

    replace_file(&source, &target).unwrap();

    assert_eq!(fs::read_to_string(&target).unwrap(), "new content");
    assert!(!source.exists());
}

#[test]
fn missing_source_keeps_existing_target() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("existing.txt");
    fs::write(&target, "last valid cache").unwrap();

    assert!(replace_file(&directory.path().join("missing.tmp"), &target).is_err());

    assert_eq!(
        fs::read_to_string(&target).expect("failed replacement must preserve the old file"),
        "last valid cache"
    );
}

#[test]
fn directory_source_cannot_replace_an_existing_file() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("directory.tmp");
    let target = directory.path().join("existing.txt");
    fs::create_dir(&source).unwrap();
    fs::write(&target, "last valid cache").unwrap();

    assert!(replace_file(&source, &target).is_err());

    assert!(source.is_dir());
    assert_eq!(fs::read_to_string(&target).unwrap(), "last valid cache");
}

#[test]
fn target_directory_is_preserved_and_failed_source_remains_for_caller_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("prepared.tmp");
    let target = directory.path().join("occupied");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("sentinel.txt"), "existing child").unwrap();
    fs::write(&source, "new content").unwrap();

    assert!(replace_file(&source, &target).is_err());

    assert_eq!(fs::read_to_string(&source).unwrap(), "new content");
    assert_eq!(
        fs::read_to_string(target.join("sentinel.txt")).unwrap(),
        "existing child"
    );
}
