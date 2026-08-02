use carrier_core::version::{check_conflicts, VersionSpec};
use semver::Version;

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

#[test]
fn parse_wildcard_matches_anything() {
    let spec = VersionSpec::parse("*").unwrap();
    assert!(spec.matches(&v("0.0.1")));
    assert!(spec.matches(&v("99.99.99")));
}

#[test]
fn parse_lower_bound_only() {
    let spec = VersionSpec::parse(">=1.0.0").unwrap();
    assert!(spec.matches(&v("1.0.0")));
    assert!(spec.matches(&v("2.0.0")));
    assert!(!spec.matches(&v("0.9.9")));
}

#[test]
fn parse_range_with_comma_separated_bounds() {
    let spec = VersionSpec::parse(">=1.0.0, <2.0.0").unwrap();
    assert!(spec.matches(&v("1.5.0")));
    assert!(!spec.matches(&v("2.0.0")));
    assert!(!spec.matches(&v("0.9.0")));
}

#[test]
fn parse_caret_compatible_range() {
    let spec = VersionSpec::parse("^1.2.0").unwrap();
    assert!(spec.matches(&v("1.2.5")));
    assert!(spec.matches(&v("1.9.0")));
    assert!(!spec.matches(&v("2.0.0")));
}

#[test]
fn parse_exact_pin() {
    let spec = VersionSpec::parse("=1.2.3").unwrap();
    assert!(spec.matches(&v("1.2.3")));
    assert!(!spec.matches(&v("1.2.4")));
}

#[test]
fn parse_trims_whitespace() {
    let spec = VersionSpec::parse("  >=1.0.0  ").unwrap();
    assert!(spec.matches(&v("1.0.0")));
}

#[test]
fn parse_invalid_spec_errors() {
    assert!(VersionSpec::parse("not a version").is_err());
}

#[test]
fn display_shows_the_underlying_requirement() {
    let spec = VersionSpec::parse(">=1.0.0").unwrap();
    // Just check it round-trips through a VersionReq without panicking
    // and produces a non-empty, parseable-looking string.
    assert!(!spec.to_string().is_empty());
}

#[test]
fn resolve_picks_first_candidate_satisfying_all_specs() {
    let specs = vec![VersionSpec::parse(">=1.0.0").unwrap(), VersionSpec::parse("<2.0.0").unwrap()];
    // newest-first, as real candidate lists are expected to be sorted
    let candidates = vec![v("2.5.0"), v("1.8.0"), v("1.0.0")];
    let resolved = VersionSpec::resolve(&specs, &candidates);
    assert_eq!(resolved, Some(&v("1.8.0")));
}

#[test]
fn resolve_returns_none_when_nothing_matches() {
    let specs = vec![VersionSpec::parse(">=5.0.0").unwrap()];
    let candidates = vec![v("1.0.0"), v("2.0.0")];
    assert_eq!(VersionSpec::resolve(&specs, &candidates), None);
}

#[test]
fn check_conflicts_ok_when_satisfiable() {
    let specs = vec![VersionSpec::parse(">=1.0.0").unwrap()];
    let candidates = vec![v("1.0.0")];
    assert!(check_conflicts("pkg", &specs, &candidates).is_ok());
}

#[test]
fn check_conflicts_errors_when_unsatisfiable() {
    let specs = vec![VersionSpec::parse(">=1.0.0").unwrap(), VersionSpec::parse("<1.0.0").unwrap()];
    let candidates = vec![v("1.0.0"), v("0.9.0")];
    let err = check_conflicts("pkg", &specs, &candidates).unwrap_err();
    assert!(err.to_string().contains("pkg"));
}
