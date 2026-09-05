use carrier_native::toolchain::{
    check_cpp_fortran_mix_experimental, check_unhandled_sources, native_sources,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn scratch_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir()
        .join(format!("carrier-toolchain-test-{label}-{n}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn native_sources_includes_fortran_files() {
    let dir = scratch_dir("native-sources-fortran");
    std::fs::write(dir.join("hello.c"), "").unwrap();
    std::fs::write(dir.join("add.f90"), "").unwrap();

    let sources = native_sources(&dir).unwrap();
    assert!(sources.contains(&"add.f90".to_string()), "expected add.f90 in {sources:?}");
    assert!(sources.contains(&"hello.c".to_string()), "expected hello.c in {sources:?}");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn native_sources_excludes_headers_and_makevars() {
    let dir = scratch_dir("native-sources-excludes");
    std::fs::write(dir.join("hello.c"), "").unwrap();
    std::fs::write(dir.join("hello.h"), "").unwrap();
    std::fs::write(dir.join("Makevars"), "").unwrap();

    let sources = native_sources(&dir).unwrap();
    assert_eq!(sources, vec!["hello.c".to_string()]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn check_unhandled_sources_ok_for_c_cpp_and_fortran() {
    let dir = scratch_dir("unhandled-ok");
    std::fs::write(dir.join("hello.c"), "").unwrap();
    std::fs::write(dir.join("hello.cpp"), "").unwrap();
    std::fs::write(dir.join("hello.f90"), "").unwrap();
    std::fs::write(dir.join("hello.h"), "").unwrap();
    std::fs::write(dir.join("Makevars"), "").unwrap();

    assert!(check_unhandled_sources(&dir).is_ok());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn check_unhandled_sources_warns_but_does_not_fail_on_unknown_extension() {
    let dir = scratch_dir("unhandled-warn");
    std::fs::write(dir.join("hello.c"), "").unwrap();
    std::fs::write(dir.join("notes.py"), "").unwrap();

    // Warns to stderr, still returns Ok: unrecognized files never
    // block a build, they only get flagged.
    assert!(check_unhandled_sources(&dir).is_ok());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cpp_fortran_mix_experimental_rejects_both_in_one_dir() {
    let dir = scratch_dir("cpp-fortran-mix");
    std::fs::write(dir.join("hello.cpp"), "").unwrap();
    std::fs::write(dir.join("add.f90"), "").unwrap();

    let err = check_cpp_fortran_mix_experimental(&dir).unwrap_err();
    assert!(err.to_string().contains("C++ and Fortran"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cpp_fortran_mix_experimental_allows_cpp_alone() {
    let dir = scratch_dir("cpp-alone");
    std::fs::write(dir.join("hello.cpp"), "").unwrap();

    assert!(check_cpp_fortran_mix_experimental(&dir).is_ok());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cpp_fortran_mix_experimental_allows_fortran_alone() {
    let dir = scratch_dir("fortran-alone");
    std::fs::write(dir.join("add.f90"), "").unwrap();

    assert!(check_cpp_fortran_mix_experimental(&dir).is_ok());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cpp_fortran_mix_experimental_allows_c_and_fortran_together() {
    // The one combination Writing R Extensions actually supports:
    // C calling Fortran in the same dir.
    let dir = scratch_dir("c-fortran-ok");
    std::fs::write(dir.join("hello.c"), "").unwrap();
    std::fs::write(dir.join("add.f90"), "").unwrap();

    assert!(check_cpp_fortran_mix_experimental(&dir).is_ok());

    std::fs::remove_dir_all(&dir).ok();
}
