use std::collections::{BTreeMap, HashMap};

use anyhow::Result;

use carrier_core::carrier_toml::{CarrierToml, ModuleDep, ModuleMeta};
use carrier_core::ops::module_graph::{resolve_transitive, ModuleFetcher};

struct MockFetcher {
    by_source: HashMap<String, CarrierToml>,
}

impl ModuleFetcher for MockFetcher {
    fn fetch(&self, source: &str) -> Result<CarrierToml> {
        self.by_source
            .get(source)
            .map(toml_clone)
            .ok_or_else(|| anyhow::anyhow!("no mock module registered for '{source}'"))
    }
}

// CarrierToml doesn't derive Clone (ModuleMeta doesn't need to in
// production code), so the mock rebuilds a fresh copy per fetch call
// instead. Only used in this test file.
fn toml_clone(t: &CarrierToml) -> CarrierToml {
    CarrierToml {
        module: ModuleMeta {
            name: t.module.name.clone(),
            version: t.module.version.clone(),
            description: t.module.description.clone(),
            authors: t.module.authors.clone(),
            license: t.module.license.clone(),
            r_version: t.module.r_version.clone(),
            src: t.module.src.clone(),
        },
        package_deps: t.package_deps.clone(),
        module_deps: t.module_deps.clone(),
        native: None,
        test: None,
    }
}

fn minimal_toml(name: &str, version: &str, module_deps: Option<BTreeMap<String, ModuleDep>>) -> CarrierToml {
    CarrierToml {
        module: ModuleMeta {
            name: name.to_owned(),
            version: version.to_owned(),
            description: String::new(),
            authors: vec![],
            license: "Unknown".to_owned(),
            r_version: "4.0.0".to_owned(),
            src: None,
        },
        package_deps: None,
        module_deps,
        native: None,
        test: None,
    }
}

#[test]
fn walks_a_two_level_chain() {
    let mut b_deps = BTreeMap::new();
    b_deps.insert(
        "b".to_owned(),
        ModuleDep::Extended { version: "*".to_owned(), source: Some("gh:x/b".to_owned()) },
    );
    let root = minimal_toml("root", "0.1.0", Some(b_deps));

    let b = minimal_toml("b", "1.0.0", None);

    let fetcher = MockFetcher {
        by_source: HashMap::from([("gh:x/b".to_owned(), b)]),
    };

    let plan = resolve_transitive(&root, &fetcher).expect("resolution should succeed");
    assert_eq!(plan.modules.get("b").map(String::as_str), Some("1.0.0"));
}

#[test]
fn detects_a_cycle_instead_of_hanging() {
    let mut a_deps = BTreeMap::new();
    a_deps.insert(
        "b".to_owned(),
        ModuleDep::Extended { version: "*".to_owned(), source: Some("gh:x/b".to_owned()) },
    );
    let root = minimal_toml("a", "0.1.0", Some(a_deps.clone()));

    // b depends back on a — a cycle.
    let mut b_deps = BTreeMap::new();
    b_deps.insert(
        "a".to_owned(),
        ModuleDep::Extended { version: "*".to_owned(), source: Some("gh:x/a".to_owned()) },
    );
    let b = minimal_toml("b", "1.0.0", Some(b_deps));

    let fetcher = MockFetcher {
        by_source: HashMap::from([
            ("gh:x/b".to_owned(), b),
            ("gh:x/a".to_owned(), minimal_toml("a", "0.1.0", Some(a_deps))),
        ]),
    };

    let err = resolve_transitive(&root, &fetcher).expect_err("a cycle must error, not hang");
    assert!(err.to_string().contains("cycle"));
}

#[test]
fn missing_source_is_a_hard_error() {
    let mut deps = BTreeMap::new();
    deps.insert("b".to_owned(), ModuleDep::Simple("*".to_owned()));
    let root = minimal_toml("root", "0.1.0", Some(deps));

    let fetcher = MockFetcher { by_source: HashMap::new() };

    let err = resolve_transitive(&root, &fetcher).expect_err("no source should be a hard error");
    assert!(err.to_string().contains("no source declared"));
}
