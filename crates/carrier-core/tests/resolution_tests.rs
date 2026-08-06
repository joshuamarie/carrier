use carrier_core::carrier_toml::{ModuleDep, PackageDep, DEFAULT_CRAN_MIRROR};
use carrier_core::ops::resolve::resolve;
use std::collections::BTreeMap;

#[test]
fn resolve_with_no_deps_produces_empty_plan() {
    let plan = resolve(&None, &None).unwrap();
    assert!(plan.packages.is_empty());
    assert!(plan.modules.is_empty());
}

#[test]
fn resolve_package_dep_uses_default_mirror_when_simple() {
    let mut deps = BTreeMap::new();
    deps.insert("dplyr".to_owned(), PackageDep::Simple("*".to_owned()));

    let plan = resolve(&Some(deps), &None).unwrap();
    let resolved = &plan.packages["dplyr"];
    assert_eq!(resolved.repo, DEFAULT_CRAN_MIRROR);
    assert_eq!(resolved.version_spec, "*");
}

#[test]
fn resolve_package_dep_keeps_explicit_repo() {
    let mut deps = BTreeMap::new();
    deps.insert(
        "fable".to_owned(),
        PackageDep::Extended {
            version: "*".to_owned(),
            repo: Some("https://tidyverts.r-universe.dev/".to_owned()),
        },
    );

    let plan = resolve(&Some(deps), &None).unwrap();
    let resolved = &plan.packages["fable"];
    assert_eq!(resolved.repo, "https://tidyverts.r-universe.dev/");
}

#[test]
fn resolve_multiple_packages_are_all_present() {
    let mut deps = BTreeMap::new();
    deps.insert("dplyr".to_owned(), PackageDep::Simple(">=1.0.0".to_owned()));
    deps.insert("stringr".to_owned(), PackageDep::Simple("*".to_owned()));

    let plan = resolve(&Some(deps), &None).unwrap();
    assert_eq!(plan.packages.len(), 2);
    assert!(plan.packages.contains_key("dplyr"));
    assert!(plan.packages.contains_key("stringr"));
}

#[test]
fn resolve_module_deps_are_marked_latest() {
    let mut mods = BTreeMap::new();
    mods.insert("utils/helpers".to_owned(), ModuleDep::Simple("*".to_owned()));

    let plan = resolve(&None, &Some(mods)).unwrap();
    assert_eq!(plan.modules["utils/helpers"], "latest");
}

#[test]
fn resolve_errors_on_invalid_version_spec() {
    let mut deps = BTreeMap::new();
    deps.insert("dplyr".to_owned(), PackageDep::Simple("not-a-version-spec".to_owned()));
    assert!(resolve(&Some(deps), &None).is_err());
}
