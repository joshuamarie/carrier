use carrier_native::Backend;

#[test]
fn backend_parse_accepts_known_values_case_insensitively() {
    assert_eq!(Backend::parse("rcpp").unwrap(), Backend::Rcpp);
    assert_eq!(Backend::parse("RCPP").unwrap(), Backend::Rcpp);
    assert_eq!(Backend::parse("cpp11").unwrap(), Backend::Cpp11);
}

#[test]
fn backend_parse_rejects_unknown_value() {
    let err = Backend::parse("fortran").unwrap_err();
    assert!(err.to_string().contains("fortran"));
}

#[test]
fn backend_default_is_rcpp() {
    assert_eq!(Backend::default(), Backend::Rcpp);
}
