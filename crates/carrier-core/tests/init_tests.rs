use carrier_core::ops::init;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn unique_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("carrier-init-test-{label}-{n}-{}", std::process::id()))
}

struct Scratch(PathBuf);
impl Scratch {
    /// Wraps an already-chosen (not-yet-created) path — `init::run` is
    /// expected to create the directory itself.
    fn uncreated(path: PathBuf) -> Self {
        Self(path)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn init_creates_expected_project_layout() {
    let scratch = Scratch::uncreated(unique_dir("layout"));
    init::run("mymod", Some(scratch.path().to_str().unwrap()), None, None).unwrap();

    assert!(scratch.path().join("carrier.toml").is_file());
    assert!(scratch.path().join("README.md").is_file());
    assert!(scratch.path().join("mymod").join("__init__.r").is_file());
    assert!(scratch.path().join("mymod").join("hello.r").is_file());
    assert!(scratch.path().join("mymod").join("add.r").is_file());

    // No native code was requested — nothing compiled-code-related
    // should exist.
    assert!(!scratch.path().join("mymod").join("hook.r").exists());
    assert!(!scratch.path().join("mymod").join("c").exists());
    assert!(!scratch.path().join("mymod").join("cpp").exists());
}

#[test]
fn init_carrier_toml_contains_module_name() {
    let scratch = Scratch::uncreated(unique_dir("toml-content"));
    init::run("weathertools", Some(scratch.path().to_str().unwrap()), None, None).unwrap();

    let contents = std::fs::read_to_string(scratch.path().join("carrier.toml")).unwrap();
    assert!(contents.contains("name = \"weathertools\""));
}

#[test]
fn init_init_r_re_exports_hello_and_add() {
    let scratch = Scratch::uncreated(unique_dir("init-r"));
    init::run("mymod", Some(scratch.path().to_str().unwrap()), None, None).unwrap();

    let contents = std::fs::read_to_string(scratch.path().join("mymod").join("__init__.r")).unwrap();
    assert!(contents.contains("box::use("));
    assert!(contents.contains("./hello[hello_world]"));
    assert!(contents.contains("./add[add]"));
}

#[test]
fn init_hello_and_add_have_no_native_dependency() {
    let scratch = Scratch::uncreated(unique_dir("init-r-pure"));
    init::run("mymod", Some(scratch.path().to_str().unwrap()), None, None).unwrap();

    let hello = std::fs::read_to_string(scratch.path().join("mymod").join("hello.r")).unwrap();
    let add = std::fs::read_to_string(scratch.path().join("mymod").join("add.r")).unwrap();

    for content in [&hello, &add] {
        assert!(!content.contains("./hook"));
        assert!(!content.contains("dll$"));
        assert!(!content.contains(".Call"));
    }
}

#[test]
fn init_fails_if_directory_already_exists() {
    let scratch = Scratch::uncreated(unique_dir("already-exists"));
    std::fs::create_dir_all(scratch.path()).unwrap();

    let err = init::run("mymod", Some(scratch.path().to_str().unwrap()), None, None).unwrap_err();
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn init_defaults_dir_name_to_name_proj_suffix() {
    // No explicit dir_name — falls back to "<name>-proj" relative to CWD.
    // Run from within a scratch CWD-equivalent by using a unique name so
    // parallel test runs (which share process CWD) can't collide.
    let unique_name = format!("carrier-cwd-test-{}", std::process::id());
    let expected_dir = PathBuf::from(format!("{unique_name}-proj"));
    // Clean up any leftovers from a previous failed run before starting.
    let _ = std::fs::remove_dir_all(&expected_dir);

    init::run(&unique_name, None, None, None).unwrap();
    assert!(expected_dir.is_dir());
    assert!(expected_dir.join("carrier.toml").is_file());

    let _ = std::fs::remove_dir_all(&expected_dir);
}
