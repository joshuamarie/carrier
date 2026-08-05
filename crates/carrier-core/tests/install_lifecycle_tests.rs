use carrier_core::ops::{init, install, remove};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

// `install`/`remove` resolve their target directory from the CARRIER_LIB
// env var (see paths::resolve_install_dir). Env vars are process-global,
// and cargo runs #[test]s in this file concurrently on multiple threads
// within the same process, so every test that touches CARRIER_LIB must
// hold this lock for its full duration or they'll stomp on each other.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn unique_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("carrier-install-test-{label}-{n}-{}", std::process::id()))
}

struct Scratch(PathBuf);
impl Scratch {
    /// Reserves a unique path without creating it. For dirs that the
    /// function under test (init::run, install::run) is expected to
    /// create itself.
    fn reserved(label: &str) -> Self {
        Self(unique_dir(label))
    }
    /// Reserves and creates the dir immediately, for scratch space the
    /// test itself needs to populate before calling into carrier-core.
    fn new(label: &str) -> Self {
        let dir = unique_dir(label);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Guard that sets CARRIER_LIB for the duration of the closure and always
/// restores/clears it afterward, even on panic (via Drop).
struct CarrierLibGuard {
    previous: Option<String>,
}
impl CarrierLibGuard {
    fn set(path: &Path) -> Self {
        let previous = std::env::var("CARRIER_LIB").ok();
        std::env::set_var("CARRIER_LIB", path);
        Self { previous }
    }
}
impl Drop for CarrierLibGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(v) => std::env::set_var("CARRIER_LIB", v),
            None => std::env::remove_var("CARRIER_LIB"),
        }
    }
}

#[test]
fn install_from_dir_then_remove_round_trip() {
    let _guard = ENV_LOCK.lock().unwrap();

    // Scaffold a real module project with `carrier init`.
    let project = Scratch::reserved("project");
    init::run("roundtripmod", Some(project.path().to_str().unwrap())).unwrap();

    // Redirect the install target to a scratch "library" dir.
    let lib = Scratch::reserved("lib");
    let _env = CarrierLibGuard::set(lib.path());

    // install_deps = false → dependency install is a dry run, so this
    // never touches the network even though the project has no deps.
    // write_lock = false (this test isn't about `carrier.lock`).
    install::run(project.path().to_str().unwrap(), false, None, false).unwrap();

    let module_dir = lib.path().join("roundtripmod");
    assert!(module_dir.join("__init__.R").is_file());
    let dist_info = lib.path().join("roundtripmod-0.1.0.dist-info");
    assert!(dist_info.join("manifest.json").is_file());

    // `carrier.toml` is a project manifest, not part of the installable module
    // It must not end up in the installed tree
    assert!(!module_dir.join("carrier.toml").exists());

    remove::run("roundtripmod", true).unwrap();
    assert!(!module_dir.exists());
    // NOTE: ops::remove::run only removes the module directory — it does
    // not clean up the `<name>-<version>.dist-info` directory the way
    // install does before a reinstall. Asserting the *actual* behavior
    // here so a future change to remove.rs is caught either way; if this
    // turns out to be unintended, the fix belongs in remove.rs, not here.
    assert!(dist_info.join("manifest.json").is_file());
}

#[test]
fn reinstalling_replaces_the_previous_install() {
    let _guard = ENV_LOCK.lock().unwrap();

    let project = Scratch::reserved("project-reinstall");
    init::run("reinstallmod", Some(project.path().to_str().unwrap())).unwrap();

    let lib = Scratch::reserved("lib-reinstall");
    let _env = CarrierLibGuard::set(lib.path());

    install::run(project.path().to_str().unwrap(), false, None, false).unwrap();

    // Add a stray file directly into the installed module dir that a
    // clean reinstall should wipe out.
    let module_dir = lib.path().join("reinstallmod");
    std::fs::write(module_dir.join("stale.R"), "leftover").unwrap();
    assert!(module_dir.join("stale.R").exists());

    install::run(project.path().to_str().unwrap(), false, None, false).unwrap();
    assert!(!module_dir.join("stale.R").exists());
    assert!(module_dir.join("__init__.R").is_file());
}

#[test]
fn remove_errors_when_module_not_installed() {
    let _guard = ENV_LOCK.lock().unwrap();

    let lib = Scratch::reserved("lib-empty");
    let _env = CarrierLibGuard::set(lib.path());

    let err = remove::run("does-not-exist", true).unwrap_err();
    assert!(err.to_string().contains("not installed"));
}

#[test]
fn install_errors_on_project_without_carrier_toml() {
    let _guard = ENV_LOCK.lock().unwrap();

    let project = Scratch::new("no-toml"); // pre-created, deliberately empty
    let lib = Scratch::reserved("lib-no-toml");
    let _env = CarrierLibGuard::set(lib.path());

    let err = install::run(project.path().to_str().unwrap(), false, None, false).unwrap_err();
    assert!(err.to_string().contains("carrier.toml"));
}

#[test]
fn write_lock_with_no_package_deps_writes_nothing() {
    let _guard = ENV_LOCK.lock().unwrap();

    // carrier init's default template comments out every package_deps
    // entry, so a freshly-scaffolded project resolves to zero packages.
    // Rhis stays network-free the same way every other test here does,
    // even with `install_deps` and `write_lock` both true.
    let project = Scratch::reserved("project-write-lock-empty");
    init::run("emptydepsmod", Some(project.path().to_str().unwrap())).unwrap();

    let lib = Scratch::reserved("lib-write-lock-empty");
    let _env = CarrierLibGuard::set(lib.path());

    install::run(project.path().to_str().unwrap(), true, None, true).unwrap();

    assert!(!project.path().join("carrier.lock").exists());
}
