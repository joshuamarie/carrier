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
    let written = scaffold::scaffold(scratch.path(), "testmod", NativeLang::C, None).unwrap();

    for expected in ["src/hello.c", "src/add.c", "src/Makevars", "hook.r", "hello.r", "add.r", "__init__.r"] {
        assert!(written.iter().any(|f| f == expected), "missing {expected} in returned list");
        assert!(scratch.path().join(expected).exists(), "missing {expected} on disk");
    }
}

#[test]
fn scaffold_c_hook_has_no_rcpp_attach() {
    let scratch = TempScratchDir::new("scaffold-c-hook");
    scaffold::scaffold(scratch.path(), "testmod", NativeLang::C, None).unwrap();
    let hook = std::fs::read_to_string(scratch.path().join("hook.r")).unwrap();
    assert!(!hook.contains("box::use(Rcpp"));
}

#[test]
fn scaffold_cpp_rcpp_hook_attaches_rcpp() {
    let scratch = TempScratchDir::new("scaffold-cpp-rcpp");
    scaffold::scaffold(scratch.path(), "testmod", NativeLang::Cpp, Some(Backend::Rcpp)).unwrap();
    let hook = std::fs::read_to_string(scratch.path().join("hook.r")).unwrap();
    assert!(hook.contains("box::use(Rcpp[...])"));
}

#[test]
fn scaffold_cpp11_hook_has_no_rcpp_attach() {
    let scratch = TempScratchDir::new("scaffold-cpp11-hook");
    scaffold::scaffold(scratch.path(), "testmod", NativeLang::Cpp, Some(Backend::Cpp11)).unwrap();
    let hook = std::fs::read_to_string(scratch.path().join("hook.r")).unwrap();
    assert!(!hook.contains("box::use(Rcpp"));
}

#[test]
fn scaffold_hook_discovers_lib_dir_dynamically() {
    let scratch = TempScratchDir::new("scaffold-hook-dynamic");
    scaffold::scaffold(scratch.path(), "testmod", NativeLang::Cpp, Some(Backend::Cpp11)).unwrap();
    let hook = std::fs::read_to_string(scratch.path().join("hook.r")).unwrap();
    assert!(hook.contains("list.files"));
    assert!(hook.contains("box::file(\".lib\")"));
    assert!(!hook.contains("\".lib/cpp\""), "hook should not hardcode a folder name");
}

#[test]
fn scaffold_init_r_has_no_trailing_comma() {
    let scratch = TempScratchDir::new("scaffold-init-comma");
    scaffold::scaffold(scratch.path(), "testmod", NativeLang::C, None).unwrap();
    let init = std::fs::read_to_string(scratch.path().join("__init__.r")).unwrap();
    assert!(!init.contains(",\n)"));
}

#[test]
fn scaffold_fortran_is_rejected() {
    let scratch = TempScratchDir::new("scaffold-fortran");
    let err = scaffold::scaffold(scratch.path(), "testmod", NativeLang::Fortran, None).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("fortran"));
}

#[test]
fn scaffold_add_r_references_module_name() {
    let scratch = TempScratchDir::new("scaffold-add-r-name");
    scaffold::scaffold(scratch.path(), "modular", NativeLang::C, None).unwrap();
    let add = std::fs::read_to_string(scratch.path().join("add.r")).unwrap();
    assert!(add.contains("dlls$modular$add"), "add.r should reference dlls$modular$add, got: {add}");
    assert!(!add.contains("{{module_name}}"), "placeholder was not substituted");
}

#[test]
fn scaffold_hello_r_references_module_name() {
    let scratch = TempScratchDir::new("scaffold-hello-r-name");
    scaffold::scaffold(scratch.path(), "modular", NativeLang::C, None).unwrap();
    let hello = std::fs::read_to_string(scratch.path().join("hello.r")).unwrap();
    assert!(hello.contains("dlls$modular$hello_world"), "hello.r should reference dlls$modular$hello_world, got: {hello}");
    assert!(!hello.contains("{{module_name}}"), "placeholder was not substituted");
}

#[test]
fn scaffold_module_name_substitution_is_not_hardcoded() {
    let scratch = TempScratchDir::new("scaffold-name-varies");
    scaffold::scaffold(scratch.path(), "somethingelse", NativeLang::C, None).unwrap();
    let add = std::fs::read_to_string(scratch.path().join("add.r")).unwrap();
    assert!(add.contains("dlls$somethingelse$add"));
    assert!(!add.contains("dlls$testmod$add"));
}

#[test]
fn scaffold_native_dir_is_always_src_regardless_of_language() {
    for (label, lang, backend) in [
        ("c", NativeLang::C, None),
        ("cpp-rcpp", NativeLang::Cpp, Some(Backend::Rcpp)),
        ("cpp-cpp11", NativeLang::Cpp, Some(Backend::Cpp11)),
    ] {
        let scratch = TempScratchDir::new(&format!("scaffold-src-dir-{label}"));
        scaffold::scaffold(scratch.path(), "testmod", lang, backend).unwrap();
        assert!(scratch.path().join("src").is_dir(), "{label}: expected src/ dir");
        assert!(!scratch.path().join("c").exists(), "{label}: should not create c/");
        assert!(!scratch.path().join("cpp").exists(), "{label}: should not create cpp/");
    }
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
