use carrier_core::carrier_toml::Author;
use carrier_core::formats::{rmbx, tar};
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
        vec![Author::Simple("Jane Doe".to_owned())],
        "MIT",
        "4.0.0",
        Dependencies {
            packages: vec![PackageDepEntry { name: "dplyr".to_owned(), version: "*".to_owned(), repo: None }],
            modules: vec![],
        },
        files,
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

    tar::bundle(src.path(), src.path(), &archive_path, &manifest).unwrap();
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
}

#[test]
fn tar_read_toml_reconstructs_module_metadata() {
    let src = Scratch::new("tar-readtoml-src");
    make_fixture_src(src.path());
    let files = tar::collect_files(src.path()).unwrap();
    let manifest = fixture_manifest("readback", files);

    let archive = Scratch::new("tar-readtoml-archive");
    let archive_path = archive.path().join("readback_0.1.0.tar.gz");
    tar::bundle(src.path(), src.path(), &archive_path, &manifest).unwrap();

    let toml = tar::read_toml(&archive_path).unwrap();
    assert_eq!(toml.module.name, "readback");
    assert_eq!(toml.module.version, "0.1.0");
    assert_eq!(toml.module.license, "MIT");
    let deps = toml.package_deps.unwrap();
    assert!(deps.contains_key("dplyr"));
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

#[test]
fn rmbx_bundle_and_unpack_round_trip() {
    let src = Scratch::new("rmbx-src");
    make_fixture_src(src.path());

    let files = rmbx::collect_files(src.path()).unwrap();
    assert!(!files.iter().any(|f| f.contains(".hidden_file")));

    let manifest = fixture_manifest("zipmod", files);
    let archive = Scratch::new("rmbx-archive");
    let archive_path = archive.path().join("zipmod_0.1.0.rmbx");

    rmbx::bundle(src.path(), src.path(), &archive_path, &manifest).unwrap();
    assert!(archive_path.is_file());

    let install = Scratch::new("rmbx-install");
    rmbx::unpack(&archive_path, install.path()).unwrap();

    assert!(install.path().join("zipmod").join("__init__.R").is_file());
    assert!(install.path().join("zipmod").join("decomp").join("helpers.R").is_file());
    assert!(!install.path().join("zipmod").join(".hidden_file").exists());

    let dist_info = install.path().join("zipmod-0.1.0.dist-info");
    assert!(dist_info.join("manifest.json").is_file());
}

#[test]
fn rmbx_read_manifest_without_full_extraction() {
    let src = Scratch::new("rmbx-readmanifest-src");
    make_fixture_src(src.path());
    let files = rmbx::collect_files(src.path()).unwrap();
    let manifest = fixture_manifest("peekmod", files);

    let archive = Scratch::new("rmbx-readmanifest-archive");
    let archive_path = archive.path().join("peekmod_0.1.0.rmbx");
    rmbx::bundle(src.path(), src.path(), &archive_path, &manifest).unwrap();

    let read_back = rmbx::read_manifest(&archive_path).unwrap();
    assert_eq!(read_back.name, "peekmod");
    assert_eq!(read_back.version, "0.1.0");
    assert_eq!(read_back.license, "MIT");
}
