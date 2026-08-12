use carrier_native::{scaffold, Backend, NativeLang};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

struct TempScratchDir(PathBuf);

impl TempScratchDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir()
            .join(format!("carrier-native-test-{label}-{n}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn scaffold_c_writes_all_expected_files() {
    let scratch = TempScratchDir::new("scaffold-c");
    let written = scaffold::scaffold(scratch.path(), NativeLang::C, None, "mymod").unwrap();

    for expected in ["c/hello.c", "c/add.c", "c/Makevars", "hook.r", "hello.r", "add.r", "__init__.r"] {
        assert!(written.iter().any(|f| f == expected), "missing {expected} in returned list");
        assert!(scratch.path().join(expected).exists(), "missing {expected} on disk");
    }
}

#[test]
fn scaffold_c_hook_has_no_rcpp_attach() {
    let scratch = TempScratchDir::new("scaffold-c-hook");
    scaffold::scaffold(scratch.path(), NativeLang::C, None, "mymod").unwrap();
    let hook = std::fs::read_to_string(scratch.path().join("hook.r")).unwrap();
    assert!(!hook.contains("box::use(Rcpp"));
}

#[test]
fn scaffold_cpp_rcpp_hook_attaches_rcpp() {
    let scratch = TempScratchDir::new("scaffold-cpp-rcpp");
    scaffold::scaffold(scratch.path(), NativeLang::Cpp, Some(Backend::Rcpp), "mymod").unwrap();
    let hook = std::fs::read_to_string(scratch.path().join("hook.r")).unwrap();
    assert!(hook.contains("box::use(Rcpp[...])"));
}

#[test]
fn scaffold_cpp11_hook_has_no_rcpp_attach() {
    let scratch = TempScratchDir::new("scaffold-cpp11-hook");
    scaffold::scaffold(scratch.path(), NativeLang::Cpp, Some(Backend::Cpp11), "mymod").unwrap();
    let hook = std::fs::read_to_string(scratch.path().join("hook.r")).unwrap();
    assert!(!hook.contains("box::use(Rcpp"));
}

#[test]
fn scaffold_hook_dyn_load_uses_module_name() {
    let scratch = TempScratchDir::new("scaffold-hook-name");
    scaffold::scaffold(scratch.path(), NativeLang::C, None, "weathertools").unwrap();
    let hook = std::fs::read_to_string(scratch.path().join("hook.r")).unwrap();
    assert!(hook.contains("\"weathertools\""));
}

#[test]
fn scaffold_init_r_has_no_trailing_comma() {
    let scratch = TempScratchDir::new("scaffold-init-comma");
    scaffold::scaffold(scratch.path(), NativeLang::C, None, "mymod").unwrap();
    let init = std::fs::read_to_string(scratch.path().join("__init__.r")).unwrap();
    assert!(!init.contains(",\n)"));
}

#[test]
fn scaffold_fortran_is_rejected() {
    let scratch = TempScratchDir::new("scaffold-fortran");
    let err = scaffold::scaffold(scratch.path(), NativeLang::Fortran, None, "mymod").unwrap_err();
    assert!(err.to_string().to_lowercase().contains("fortran"));
}
