//! Uses the real `convert-proj` example module that ships at the repo
//! root as a fixture, instead of a synthetic one. This is the actual
//! deliverable a user would bundle/install with `carrier`, so a passing
//! suite here means the example in the repo genuinely still works with
//! the current carrier-core code — not just a stand-in.
//!
//! Values are read from convert-proj's real `carrier.toml` at test time
//! (module name, version, ...) rather than hardcoded, so this doesn't
//! silently go stale if that file changes.

use carrier_core::carrier_toml::CarrierToml;
use carrier_core::formats::tar;
use carrier_core::manifest::Manifest;
use carrier_core::ops::{install, remove};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

// install/remove resolve their target dir from CARRIER_LIB (env vars are
// process-global); see the same guard pattern in install_lifecycle_tests.rs.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Path to the real convert-proj/ directory at the repo root, resolved
/// relative to this crate (crates/carrier-core) rather than the process's
/// current working directory, so `cargo test` works the same no matter
/// where it's invoked from.
fn convert_proj_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../convert-proj")
}

fn unique_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("carrier-convertproj-test-{label}-{n}-{}", std::process::id()))
}

struct Scratch(PathBuf);
impl Scratch {
    fn reserved(label: &str) -> Self {
        Self(unique_dir(label))
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

/// Sanity check that the fixture is actually present before trusting any
/// other test's results — if this fails, the other tests here are
/// meaningless, not passing-by-accident.
#[test]
fn convert_proj_fixture_exists_with_carrier_toml() {
    let dir = convert_proj_dir();
    assert!(dir.is_dir(), "convert-proj not found at {}", dir.display());
    assert!(dir.join("carrier.toml").is_file());
}

#[test]
fn convert_proj_carrier_toml_parses_and_resolves_src() {
    let dir = convert_proj_dir();
    let toml = CarrierToml::from_dir(&dir).expect("convert-proj/carrier.toml should parse");
    let src = toml.resolve_src_dir(&dir).expect("src dir should resolve");

    // The module's entry point must exist, per carrier's own contract.
    assert!(src.join("__init__.R").is_file());
}

#[test]
fn convert_proj_bundles_as_tar_gz_with_matching_metadata() {
    let dir = convert_proj_dir();
    let toml = CarrierToml::from_dir(&dir).unwrap();
    let src = toml.resolve_src_dir(&dir).unwrap();

    let archive_dir = Scratch::reserved("tar-archive");
    std::fs::create_dir_all(archive_dir.path()).unwrap();
    let archive_path = archive_dir.path().join("convert.tar.gz");

    let files = tar::collect_files(&src).unwrap();
    assert!(!files.is_empty(), "convert-proj source tree should not be empty");

    let manifest = Manifest::new(
        &toml.module.name,
        &toml.module.version,
        &toml.module.description,
        toml.module.authors.clone(),
        &toml.module.license,
        &toml.module.r_version,
        Default::default(),
        files,
    );

    tar::bundle(&src, &dir, &archive_path, &manifest).unwrap();
    assert!(archive_path.is_file());

    let read_back = tar::read_toml(&archive_path).unwrap();
    assert_eq!(read_back.module.name, toml.module.name);
    assert_eq!(read_back.module.version, toml.module.version);
}

#[test]
fn convert_proj_installs_via_carrier_install_run() {
    let _guard = ENV_LOCK.lock().unwrap();

    let dir = convert_proj_dir();
    let toml = CarrierToml::from_dir(&dir).expect("convert-proj/carrier.toml should parse");

    let lib = Scratch::reserved("lib");
    let _env = CarrierLibGuard::set(lib.path());

    // install_deps = false → dependency install stays a dry run, so this
    // never touches the network regardless of what convert-proj declares
    // under [package_deps].
    install::run(dir.to_str().unwrap(), false, None, false).expect("installing convert-proj should succeed");

    let module_dir = lib.path().join(&toml.module.name);
    assert!(module_dir.join("__init__.R").is_file());

    // The submodules actually present in convert-proj (mass/, temp/) must
    // survive the bundle → install round trip intact.
    assert!(module_dir.join("mass").join("__init__.R").is_file());
    assert!(module_dir.join("mass").join("basic_mass.R").is_file());
    assert!(module_dir.join("mass").join("const.R").is_file());
    assert!(module_dir.join("mass").join("conversions.R").is_file());
    assert!(module_dir.join("mass").join("cross_system_mass.R").is_file());
    assert!(module_dir.join("mass").join("imperial_mass.R").is_file());
    assert!(module_dir.join("mass").join("specialty.R").is_file());
    assert!(module_dir.join("temp").join("__init__.R").is_file());
    assert!(module_dir.join("temp").join("conversions.R").is_file());

    // carrier.toml and README.md are project files, not module files —
    // must not leak into the installed tree.
    assert!(!module_dir.join("carrier.toml").exists());
    assert!(!module_dir.join("README.md").exists());

    let dist_info = lib.path().join(format!("{}-{}.dist-info", toml.module.name, toml.module.version));
    assert!(dist_info.join("manifest.json").is_file());
    let manifest_json = std::fs::read_to_string(dist_info.join("manifest.json")).unwrap();
    let manifest = Manifest::from_json(&manifest_json).unwrap();
    assert_eq!(manifest.name, toml.module.name);
    assert_eq!(manifest.version, toml.module.version);

    remove::run(&toml.module.name, true).expect("removing convert-proj should succeed");
    assert!(!module_dir.exists());
}
