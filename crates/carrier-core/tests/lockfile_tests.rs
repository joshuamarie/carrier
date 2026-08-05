use carrier_core::lockfile::{read, write, LOCK_FILE_NAME};
use semver::Version;
use std::collections::BTreeMap;

#[test]
fn reading_a_missing_lock_is_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(read(dir.path()).unwrap().is_none());
}

#[test]
fn write_then_read_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let mut resolved = BTreeMap::new();
    resolved.insert(
        "purrr".to_owned(),
        (Version::parse("1.2.2").unwrap(), "https://cloud.r-project.org".to_owned()),
    );
    resolved.insert(
        "rlang".to_owned(),
        (Version::parse("1.3.0").unwrap(), "https://cloud.r-project.org".to_owned()),
    );

    write(dir.path(), &resolved).unwrap();
    let lock = read(dir.path()).unwrap().expect("lock should exist after write");

    let (version, repo) = lock.locked_version("purrr").unwrap().unwrap();
    assert_eq!(version, Version::parse("1.2.2").unwrap());
    assert_eq!(repo, "https://cloud.r-project.org");
}

#[test]
fn locked_version_is_none_for_an_unlisted_package() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), &BTreeMap::new()).unwrap();
    let lock = read(dir.path()).unwrap().unwrap();
    assert!(lock.locked_version("nonexistent").unwrap().is_none());
}

#[test]
fn an_invalid_version_in_the_lock_is_a_hard_error_not_a_silent_skip() {
    // A lock the tool can't trust is worse than no lock at all — it
    // would look authoritative while quietly not being enforced.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(LOCK_FILE_NAME),
        "version = 1\n\n[[package]]\nname = \"purrr\"\nversion = \"not-a-version\"\nrepo = \"https://cloud.r-project.org\"\n",
    )
    .unwrap();

    let lock = read(dir.path()).unwrap().unwrap();
    assert!(lock.locked_version("purrr").is_err());
}

#[test]
fn a_malformed_lock_file_fails_to_read_rather_than_being_ignored() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(LOCK_FILE_NAME), "this is not valid toml {{{").unwrap();
    assert!(read(dir.path()).is_err());
}

#[test]
fn written_packages_are_sorted_for_a_stable_diff() {
    let dir = tempfile::tempdir().unwrap();
    let mut resolved = BTreeMap::new();
    resolved.insert("zzz_pkg".to_owned(), (Version::parse("1.0.0").unwrap(), "repo".to_owned()));
    resolved.insert("aaa_pkg".to_owned(), (Version::parse("1.0.0").unwrap(), "repo".to_owned()));

    write(dir.path(), &resolved).unwrap();
    let contents = std::fs::read_to_string(dir.path().join(LOCK_FILE_NAME)).unwrap();
    let aaa_pos = contents.find("aaa_pkg").unwrap();
    let zzz_pos = contents.find("zzz_pkg").unwrap();
    assert!(aaa_pos < zzz_pos, "expected aaa_pkg to appear before zzz_pkg in the written file");
}
