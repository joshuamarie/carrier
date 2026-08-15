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
    let written = scaffold::scaffold(scratch.path(), NativeLang::C, None).unwrap();

    for expected in ["c/hello.c", "c/add.c", "c/Makevars", "hook.r", "hello.r", "add.r", "__init__.r"] {
        assert!(written.iter().any(|f| f == expected), "missing {expected} in returned list");
        assert!(scratch.path().join(expected).exists(), "missing {expected} on disk");
    }
}

#[test]
fn scaffold_c_hook_has_no_rcpp_attach() {
    let scratch = TempScratchDir::new("scaffold-c-hook");
    scaffold::scaffold(scratch.path(), NativeLang::C, None).unwrap();
    let hook = std::fs::read_to_string(scratch.path().join("hook.r")).unwrap();
    assert!(!hook.contains("box::use(Rcpp"));
}

#[test]
fn scaffold_cpp_rcpp_hook_attaches_rcpp() {
    let scratch = TempScratchDir::new("scaffold-cpp-rcpp");
    scaffold::scaffold(scratch.path(), NativeLang::Cpp, Some(Backend::Rcpp)).unwrap();
    let hook = std::fs::read_to_string(scratch.path().join("hook.r")).unwrap();
    assert!(hook.contains("box::use(Rcpp[...])"));
}

#[test]
fn scaffold_cpp11_hook_has_no_rcpp_attach() {
    let scratch = TempScratchDir::new("scaffold-cpp11-hook");
    scaffold::scaffold(scratch.path(), NativeLang::Cpp, Some(Backend::Cpp11)).unwrap();
    let hook = std::fs::read_to_string(scratch.path().join("hook.r")).unwrap();
    assert!(!hook.contains("box::use(Rcpp"));
}

#[test]
fn scaffold_hook_dyn_load_uses_native_dir_name() {
    let scratch = TempScratchDir::new("scaffold-hook-name");
    scaffold::scaffold(scratch.path(), NativeLang::Cpp, Some(Backend::Cpp11)).unwrap();
    let hook = std::fs::read_to_string(scratch.path().join("hook.r")).unwrap();
    assert!(hook.contains("\".lib/cpp\""));
}

#[test]
fn scaffold_init_r_has_no_trailing_comma() {
    let scratch = TempScratchDir::new("scaffold-init-comma");
    scaffold::scaffold(scratch.path(), NativeLang::C, None).unwrap();
    let init = std::fs::read_to_string(scratch.path().join("__init__.r")).unwrap();
    assert!(!init.contains(",\n)"));
}

#[test]
fn scaffold_fortran_is_rejected() {
    let scratch = TempScratchDir::new("scaffold-fortran");
    let err = scaffold::scaffold(scratch.path(), NativeLang::Fortran, None).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("fortran"));
}

#[test]
fn has_native_src_true_for_bare_c_file_no_makevars() {
    let scratch = TempScratchDir::new("detect-no-makevars");
    std::fs::write(scratch.path().join("hello.c"), "// no makevars here").unwrap();
    assert!(carrier_native::detect::has_native_src(scratch.path()));
}

#[test]
fn has_native_src_false_for_empty_dir() {
    let scratch = TempScratchDir::new("detect-empty");
    assert!(!carrier_native::detect::has_native_src(scratch.path()));
}

#[test]
fn scaffold_pure_r_writes_expected_files_only() {
    let scratch = TempScratchDir::new("scaffold-pure-r");
    let written = scaffold::scaffold_pure_r(scratch.path()).unwrap();

    for expected in ["hello.r", "add.r", "__init__.r"] {
        assert!(written.iter().any(|f| f == expected), "missing {expected} in returned list");
        assert!(scratch.path().join(expected).exists(), "missing {expected} on disk");
    }

    // No native dir, no hook.r — that's the whole point of this path.
    assert!(!scratch.path().join("hook.r").exists());
    assert!(!scratch.path().join("c").exists());
    assert!(!scratch.path().join("cpp").exists());
}

#[test]
fn scaffold_pure_r_functions_have_no_native_dependency() {
    let scratch = TempScratchDir::new("scaffold-pure-r-no-dll");
    scaffold::scaffold_pure_r(scratch.path()).unwrap();

    let hello = std::fs::read_to_string(scratch.path().join("hello.r")).unwrap();
    let add = std::fs::read_to_string(scratch.path().join("add.r")).unwrap();

    for content in [&hello, &add] {
        assert!(!content.contains("./hook"));
        assert!(!content.contains("dll$"));
        assert!(!content.contains(".Call"));
    }
}

#[test]
fn scaffold_pure_r_init_has_no_trailing_comma() {
    let scratch = TempScratchDir::new("scaffold-pure-r-init-comma");
    scaffold::scaffold_pure_r(scratch.path()).unwrap();
    let init = std::fs::read_to_string(scratch.path().join("__init__.r")).unwrap();
    assert!(!init.contains(",\n)"));
}
