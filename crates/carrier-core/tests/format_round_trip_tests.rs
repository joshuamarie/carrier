use carrier_core::carrier_toml::{Author, TestConfig};
use carrier_core::formats::tar;
use carrier_core::lockfile::LockedPackage;
use carrier_core::manifest::{Dependencies, Manifest, PackageDepEntry};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

fn unique_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("carrier-fmt-test-{label}-{n}-{}", std::process::id()))
}

struct Scratch(PathBuf);
impl Scratch {
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

/// Build a small fixture module source tree:
/// <src>/__init__.R
/// <src>/decomp/helpers.R
/// <src>/.hidden_file   (should be excluded from the bundle)
fn make_fixture_src(dir: &Path) {
    std::fs::create_dir_all(dir.join("decomp")).unwrap();
    std::fs::write(dir.join("__init__.R"), "#' @export\nbox::use()\n").unwrap();
    std::fs::write(dir.join("decomp").join("helpers.R"), "helper <- function() 1\n").unwrap();
    std::fs::write(dir.join(".hidden_file"), "should not be bundled").unwrap();
}

fn fixture_manifest(name: &str, files: Vec<String>) -> Manifest {
    Manifest::new(
        name,
        "0.1.0",
        "A test module",
        vec![
            Author::Simple("Jane Doe".to_owned()),
            Author::Extended {
                name: "John Smith".to_owned(),
                email: Some("john@example.com".to_owned()),
                url: None,
                orcid: None,
            },
        ],
        "MIT",
        "4.0.0",
        Dependencies {
            packages: vec![PackageDepEntry { name: "dplyr".to_owned(), version: "*".to_owned(), repo: None }],
            modules: vec![],
        },
        files,
        Some(vec![LockedPackage {
            name: "dplyr".to_owned(),
            version: "1.1.4".to_owned(),
            repo: "https://cloud.r-project.org".to_owned(),
        }]),
        Some(TestConfig {
            framework: "testthat".to_owned(),
            dir: Some("tests".to_owned()),
        }),
    )
}

#[test]
fn tar_bundle_and_unpack_round_trip() {
    let src = Scratch::new("tar-src");
    make_fixture_src(src.path());

    let files = tar::collect_files(src.path()).unwrap();
    // Hidden files must not be picked up for the manifest's file list.
    assert!(!files.iter().any(|f| f.contains(".hidden_file")));

    let manifest = fixture_manifest("mymod", files);
    let archive = Scratch::new("tar-archive");
    let archive_path = archive.path().join("mymod_0.1.0.tar.gz");

    tar::bundle(src.path(), src.path(), &archive_path, &manifest, &[], &[]).unwrap();
    assert!(archive_path.is_file());

    let install = Scratch::new("tar-install");
    tar::unpack(&archive_path, install.path(), "mymod", "0.1.0").unwrap();

    // Module files land directly under <install_dir>/<name>/...
    assert!(install.path().join("mymod").join("__init__.R").is_file());
    assert!(install.path().join("mymod").join("decomp").join("helpers.R").is_file());
    // The hidden file was never bundled, so it can't appear in the install.
    assert!(!install.path().join("mymod").join(".hidden_file").exists());

    // manifest.json lands in the dist-info dir, not inside the module dir.
    let dist_info = install.path().join("mymod-0.1.0.dist-info");
    assert!(dist_info.join("manifest.json").is_file());
    assert!(!install.path().join("mymod").join("manifest.json").exists());

    let manifest_json = std::fs::read_to_string(dist_info.join("manifest.json")).unwrap();
    let read_back = Manifest::from_json(&manifest_json).unwrap();
    assert_eq!(read_back.name, "mymod");
    assert_eq!(read_back.dependencies.packages[0].name, "dplyr");

    let locked = read_back.locked_packages.expect("locked_packages should survive the round trip");
    assert_eq!(locked[0].name, "dplyr");
    assert_eq!(locked[0].version, "1.1.4");

    let test_cfg = read_back.test.expect("test config should survive the round trip");
    assert_eq!(test_cfg.framework, "testthat");
}

#[test]
fn tar_read_toml_reconstructs_module_metadata() {
    let src = Scratch::new("tar-readtoml-src");
    make_fixture_src(src.path());
    let files = tar::collect_files(src.path()).unwrap();
    let manifest = fixture_manifest("readback", files);

    let archive = Scratch::new("tar-readtoml-archive");
    let archive_path = archive.path().join("readback_0.1.0.tar.gz");
    tar::bundle(src.path(), src.path(), &archive_path, &manifest, &[], &[]).unwrap();

    let toml = tar::read_toml(&archive_path).unwrap();
    assert_eq!(toml.module.name, "readback");
    assert_eq!(toml.module.version, "0.1.0");
    assert_eq!(toml.module.license, "MIT");
    let deps = toml.package_deps.unwrap();
    assert!(deps.contains_key("dplyr"));

    let test_cfg = toml.test.expect("test config should survive read_toml's reconstruction");
    assert_eq!(test_cfg.framework, "testthat");
    assert_eq!(test_cfg.dir.as_deref(), Some("tests"));

    let john = toml.module.authors.iter().find(|a| a.name() == "John Smith")
        .expect("Extended author should survive read_toml's reconstruction");
    assert_eq!(john.email(), Some("john@example.com"));
}

#[test]
fn tar_user_file_named_manifest_json_survives_bundling() {
    let src = Scratch::new("tar-collision-src");
    make_fixture_src(src.path());
    // A module file that happens to share a name with carrier's own
    // generated manifest. This must not collide with it on write, and
    // must not be misrouted into .dist-info on unpack.
    std::fs::write(src.path().join("manifest.json"), "user's own data, not carrier's").unwrap();

    let files = tar::collect_files(src.path()).unwrap();
    let manifest = fixture_manifest("collisionmod", files);

    let archive = Scratch::new("tar-collision-archive");
    let archive_path = archive.path().join("collisionmod_0.1.0.tar.gz");
    tar::bundle(src.path(), src.path(), &archive_path, &manifest, &[], &[]).unwrap();

    let install = Scratch::new("tar-collision-install");
    tar::unpack(&archive_path, install.path(), "collisionmod", "0.1.0").unwrap();

    // The user's file is installed as ordinary module content, untouched.
    let user_file = install.path().join("collisionmod").join("manifest.json");
    assert_eq!(std::fs::read_to_string(&user_file).unwrap(), "user's own data, not carrier's");

    // carrier's own manifest still lands in .dist-info, and is still valid.
    let dist_info = install.path().join("collisionmod-0.1.0.dist-info");
    let carrier_manifest = std::fs::read_to_string(dist_info.join("manifest.json")).unwrap();
    let parsed = Manifest::from_json(&carrier_manifest).unwrap();
    assert_eq!(parsed.name, "collisionmod");
}

#[test]
fn tar_read_toml_errors_on_non_carrier_archive() {
    let scratch = Scratch::new("tar-not-carrier");
    let not_carrier = scratch.path().join("plain.tar.gz");

    // A tarball with no manifest.json inside at all.
    let file = std::fs::File::create(&not_carrier).unwrap();
    let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut archive = ::tar::Builder::new(enc);
    let readme = scratch.path().join("README.md");
    std::fs::write(&readme, "hello").unwrap();
    archive.append_path_with_name(&readme, "top/README.md").unwrap();
    archive.finish().unwrap();

    assert!(tar::read_toml(&not_carrier).is_err());
}
