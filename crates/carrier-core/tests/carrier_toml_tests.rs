use carrier_core::carrier_toml::{Author, CarrierToml, ModuleMeta, PackageDep, DEFAULT_CRAN_MIRROR};
use carrier_native::{Backend, NativeLang};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

// Minimal throwaway-directory helper so these tests don't need the `tempfile` crate
// Each test gets its own unique dir under the OS
// temp dir and cleans up after itself.
struct TempScratchDir(PathBuf);

impl TempScratchDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("carrier-toml-test-{label}-{n}-{}", std::process::id()));
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

fn module_meta(name: &str, src: Option<&str>) -> ModuleMeta {
    ModuleMeta {
        name: name.to_owned(),
        version: "0.1.0".to_owned(),
        description: String::new(),
        authors: vec![Author::Simple("Jane Doe".to_owned())],
        license: "MIT".to_owned(),
        r_version: "4.0.0".to_owned(),
        src: src.map(|s| s.to_owned()),
    }
}

#[derive(Deserialize)]
struct AuthorsWrapper {
    authors: Vec<Author>,
}

#[test]
fn author_simple_string_deserializes() {
    let wrapper: AuthorsWrapper = toml::from_str(r#"authors = ["Jane Doe"]"#).unwrap();
    assert_eq!(wrapper.authors[0].name(), "Jane Doe");
    assert_eq!(wrapper.authors[0].email(), None);
}

#[test]
fn author_extended_table_deserializes() {
    let toml_str = r#"
        authors = [
            { name = "Jane Doe", email = "jane@example.com" },
        ]
    "#;
    let wrapper: AuthorsWrapper = toml::from_str(toml_str).unwrap();
    let author = &wrapper.authors[0];
    assert_eq!(author.name(), "Jane Doe");
    assert_eq!(author.email(), Some("jane@example.com"));
    assert_eq!(author.url(), None);
    assert_eq!(author.orcid(), None);
}

#[test]
fn author_mixed_simple_and_extended_in_one_list() {
    let toml_str = r#"
        authors = [
            "Jane Doe",
            { name = "John Smith", email = "john@example.com" },
        ]
    "#;
    let wrapper: AuthorsWrapper = toml::from_str(toml_str).unwrap();
    assert_eq!(wrapper.authors.len(), 2);
    assert_eq!(wrapper.authors[0].name(), "Jane Doe");
    assert_eq!(wrapper.authors[1].name(), "John Smith");
}

#[test]
fn author_simple_display() {
    let author = Author::Simple("Jane Doe".to_owned());
    assert_eq!(author.to_string(), "Jane Doe");
}

#[test]
fn author_extended_display_includes_email_and_url() {
    let author = Author::Extended {
        name: "Jane Doe".to_owned(),
        email: Some("jane@example.com".to_owned()),
        url: Some("https://example.com".to_owned()),
        orcid: None,
    };
    assert_eq!(author.to_string(), "Jane Doe <jane@example.com> (https://example.com)");
}

#[test]
fn package_dep_simple_uses_default_mirror() {
    let dep = PackageDep::Simple("*".to_owned());
    assert_eq!(dep.version(), "*");
    assert_eq!(dep.repo(), DEFAULT_CRAN_MIRROR);
}

#[test]
fn package_dep_extended_with_explicit_repo() {
    let dep = PackageDep::Extended {
        version: "*".to_owned(),
        repo: Some("https://tidyverts.r-universe.dev/".to_owned()),
    };
    assert_eq!(dep.repo(), "https://tidyverts.r-universe.dev/");
}

#[test]
fn package_dep_extended_without_repo_falls_back_to_default() {
    let dep = PackageDep::Extended { version: ">=1.0.0".to_owned(), repo: None };
    assert_eq!(dep.repo(), DEFAULT_CRAN_MIRROR);
}

#[test]
fn resolve_src_dir_defaults_to_module_name() {
    let scratch = TempScratchDir::new("default-name");
    let src_dir = scratch.path().join("mymod");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("__init__.R"), "").unwrap();

    let toml = CarrierToml {
        module: module_meta("mymod", None),
        package_deps: None,
        module_deps: None,
        native: None,
        test: None,
    };

    let resolved = toml.resolve_src_dir(scratch.path()).unwrap();
    assert_eq!(resolved, src_dir);
}

#[test]
fn resolve_src_dir_errors_when_default_dir_missing() {
    let scratch = TempScratchDir::new("missing-default");
    let toml = CarrierToml {
        module: module_meta("mymod", None),
        package_deps: None,
        module_deps: None,
        native: None,
        test: None,
    };

    let err = toml.resolve_src_dir(scratch.path()).unwrap_err();
    assert!(err.to_string().contains("mymod"));
}

#[test]
fn resolve_src_dir_errors_when_init_r_missing() {
    let scratch = TempScratchDir::new("missing-init");
    std::fs::create_dir_all(scratch.path().join("mymod")).unwrap();

    let toml = CarrierToml {
        module: module_meta("mymod", None),
        package_deps: None,
        module_deps: None,
        native: None,
        test: None,
    };

    let err = toml.resolve_src_dir(scratch.path()).unwrap_err();
    assert!(err.to_string().contains("__init__.R"));
}

#[test]
fn resolve_src_dir_uses_explicit_src_override() {
    let scratch = TempScratchDir::new("explicit-src");
    let src_dir = scratch.path().join("custom_source");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("__init__.R"), "").unwrap();

    let toml = CarrierToml {
        module: module_meta("mymod", Some("custom_source")),
        package_deps: None,
        module_deps: None,
        native: None,
        test: None,
    };

    let resolved = toml.resolve_src_dir(scratch.path()).unwrap();
    assert_eq!(resolved, src_dir);
}

#[test]
fn resolve_src_dir_errors_when_explicit_src_not_a_directory() {
    let scratch = TempScratchDir::new("explicit-src-not-dir");
    let toml = CarrierToml {
        module: module_meta("mymod", Some("does_not_exist")),
        package_deps: None,
        module_deps: None,
        native: None,
        test: None,
    };

    assert!(toml.resolve_src_dir(scratch.path()).is_err());
}

#[test]
fn default_template_contains_module_name_and_parses_as_toml() {
    // let template = CarrierToml::default_template("mymod");
    let template = CarrierToml::default_template("mymod", Some((NativeLang::C, None)));
    assert!(template.contains("name = \"mymod\""));
    let parsed: CarrierToml = toml::from_str(&template).unwrap();
    assert_eq!(parsed.module.name, "mymod");
}

#[test]
fn default_template_none_leaves_native_block_fully_commented() {
    let template = CarrierToml::default_template("mymod", None);
    assert!(template.contains("# path = \"native/\""));
    assert!(template.contains("# build_deps = { Rcpp = \"*\" }"));
    assert!(!template.lines().any(|l| l.trim_start().starts_with("path =")));
    assert!(!template.lines().any(|l| l.trim_start().starts_with("build_deps =")));
}

#[test]
fn default_template_c_sets_path_under_module_dir_no_build_deps() {
    let template = CarrierToml::default_template("mymod", Some((NativeLang::C, None)));
    assert!(template.contains("path = \"c/\""));
    assert!(template.contains("# build_deps = { Rcpp = \"*\" }"));
}

#[test]
fn default_template_cpp_rcpp_sets_path_and_build_deps() {
    let template = CarrierToml::default_template(
        "mymod",
        Some((NativeLang::Cpp, Some(Backend::Rcpp))),
    );
    assert!(template.contains("path = \"cpp/\""));
    assert!(template.lines().any(|l| l.trim_start() == "build_deps = { Rcpp = \"*\" }"));
}

#[test]
fn default_template_cpp_omitted_backend_defaults_to_rcpp() {
    let template = CarrierToml::default_template("mymod", Some((NativeLang::Cpp, None)));
    assert!(template.lines().any(|l| l.trim_start() == "build_deps = { Rcpp = \"*\" }"));
}

#[test]
fn default_template_cpp11_sets_cpp11_build_deps() {
    let template = CarrierToml::default_template(
        "mymod",
        Some((NativeLang::Cpp, Some(Backend::Cpp11))),
    );
    assert!(template.lines().any(|l| l.trim_start() == "build_deps = { cpp11 = \"*\" }"));
}

#[test]
fn default_template_all_native_variants_parse_as_valid_toml() {
    for native in [
        None,
        Some((NativeLang::C, None)),
        Some((NativeLang::Cpp, Some(Backend::Rcpp))),
        Some((NativeLang::Cpp, Some(Backend::Cpp11))),
    ] {
        let template = CarrierToml::default_template("mymod", native);
        assert!(toml::from_str::<CarrierToml>(&template).is_ok());
    }
}
