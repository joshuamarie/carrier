//! Integration test for `carrier-native`, run manually — it needs R
//! (with `Rcpp` installed) and a C++ toolchain on PATH, neither of
//! which CI can be assumed to have:
//!
//!   cargo test -p carrier-native -- --ignored --nocapture
//!
//! The fixture module (a real, working `{box}` module with compiled
//! Rcpp code, copied from `native-demo`) lives entirely as inline
//! string constants below, written out to a `TempDir` at test start
//! and cleaned up automatically when it drops. Deliberately not a
//! checked-in `tests/fixtures/` folder — that was a repeated source of
//! "did the files actually land on disk" friction with no upside; a
//! self-contained test file has nothing external to get out of sync.
//!
//! The module's correct output —
//! `rolling_mean(c(1..10), window = 3)` => `NA NA 2 3 4 5 6 7 8 9` —
//! is already known independently of this crate (see
//! `native-demo/README.md`), so a failure here is unambiguously
//! `carrier-native`'s fault, not the fixture's.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

const INIT_R: &str = r#"box::use(
    Rcpp[...]
)

dyn.load(box::file(paste0("stats_native", .Platform$dynlib.ext)))

#' Simple moving average over `x` with the given `window` size
#' @export
rolling_mean = function(x, window) {
    .Call("rolling_mean", as.double(x), as.integer(window))
}

#' Pairwise Euclidean distances between rows of a numeric matrix
#' @export
pairwise_distances = function(m) {
    .Call("pairwise_distances", as.matrix(m))
}

.on_unload = function(ns) {
    dyn.unload(box::file(paste0("stats_native", .Platform$dynlib.ext)))
}
"#;

const MAKEVARS: &str =
    "PKG_CPPFLAGS = -I\"$(shell \"$(R_HOME)/bin/Rscript\" -e \"cat(system.file('include', package = 'Rcpp'))\")\"\n";

const STATS_NATIVE_H: &str = r#"#ifndef STATS_NATIVE_H
#define STATS_NATIVE_H

#include <Rcpp.h>

extern "C" SEXP rolling_mean(SEXP x, SEXP window);
extern "C" SEXP pairwise_distances(SEXP m);

#endif
"#;

const ROLLING_MEAN_CPP: &str = r#"#include "stats_native.h"

extern "C" SEXP rolling_mean(SEXP x_, SEXP window_) {
    Rcpp::NumericVector x(x_);
    int window = Rcpp::as<int>(window_);

    if (window < 1 || window > x.size()) {
        Rcpp::stop("window must be between 1 and length(x)");
    }

    Rcpp::NumericVector out(x.size(), NA_REAL);
    double sum = 0.0;

    for (int i = 0; i < x.size(); i++) {
        sum += x[i];
        if (i >= window) {
            sum -= x[i - window];
        }
        if (i >= window - 1) {
            out[i] = sum / window;
        }
    }

    return out;
}
"#;

const PAIRWISE_DISTANCES_CPP: &str = r#"#include "stats_native.h"
#include <cmath>

extern "C" SEXP pairwise_distances(SEXP m_) {
    Rcpp::NumericMatrix m(m_);
    int n = m.nrow();
    int p = m.ncol();

    Rcpp::NumericMatrix out(n, n);

    for (int i = 0; i < n; i++) {
        for (int j = i + 1; j < n; j++) {
            double sum_sq = 0.0;
            for (int k = 0; k < p; k++) {
                double diff = m(i, k) - m(j, k);
                sum_sq += diff * diff;
            }
            double dist = std::sqrt(sum_sq);
            out(i, j) = dist;
            out(j, i) = dist;
        }
    }

    return out;
}
"#;

/// Writes the fixture module into a fresh temp directory and returns
/// it. The `TempDir` must stay alive for as long as the module is
/// needed — it deletes itself on drop.
fn write_fixture() -> TempDir {
    let tmp = TempDir::new().expect("failed to create temp directory for fixture");
    let module_dir = tmp.path().join("stats_native");
    let src_dir = module_dir.join("src");
    std::fs::create_dir_all(&src_dir).expect("failed to create fixture src/ dir");

    std::fs::write(module_dir.join("__init__.R"), INIT_R).unwrap();
    std::fs::write(src_dir.join("Makevars"), MAKEVARS).unwrap();
    std::fs::write(src_dir.join("stats_native.h"), STATS_NATIVE_H).unwrap();
    std::fs::write(src_dir.join("rolling_mean.cpp"), ROLLING_MEAN_CPP).unwrap();
    std::fs::write(src_dir.join("pairwise_distances.cpp"), PAIRWISE_DISTANCES_CPP).unwrap();

    tmp
}

#[test]
#[ignore = "requires R (with Rcpp installed) and a C++ toolchain on PATH"]
fn builds_caches_and_loads_stats_native() {
    let tmp = write_fixture();
    let module_dir = tmp.path().join("stats_native");
    let native_dir = module_dir.join("src");

    assert!(
        carrier_native::has_native_src(&native_dir),
        "expected a Makevars under {}",
        native_dir.display()
    );

    let first = carrier_native::build(&module_dir, &native_dir, "stats_native")
        .expect("first build() should succeed");
    assert!(
        first.artifact_path.exists(),
        "artifact should exist on disk after build(): {}",
        first.artifact_path.display()
    );

    // Re-running build() with unchanged source should hit the cache,
    // not invoke R CMD SHLIB again.
    let second = carrier_native::build(&module_dir, &native_dir, "stats_native")
        .expect("second build() should succeed");
    assert!(second.from_cache, "second build() should be a cache hit");
    assert_eq!(
        first.source_hash, second.source_hash,
        "source hash should be stable across identical builds"
    );

    // The part a compile-only check would miss: the artifact has to
    // actually load and run correctly in R, including its own runtime
    // deps (Rcpp) — see the earlier `Rcpp_precious_remove` failure that
    // a manual dyn.load() without library(Rcpp) first ran into.
    assert_rolling_mean_matches(&first.artifact_path);

    // tmp deletes itself here, on drop — nothing left behind either way.
}

fn assert_rolling_mean_matches(artifact_path: &Path) {
    let path_str = artifact_path.to_string_lossy().replace('\\', "/");
    let expr = format!(
        "library(Rcpp); dyn.load('{path_str}'); \
         cat(paste(.Call('rolling_mean', c(1,2,3,4,5,6,7,8,9,10), 3L), collapse = ','))"
    );

    let output = Command::new("Rscript")
        .arg("-e")
        .arg(&expr)
        .output()
        .expect("failed to run Rscript — is R installed and on PATH?");

    assert!(
        output.status.success(),
        "Rscript failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = "NA,NA,2,3,4,5,6,7,8,9";
    assert!(
        stdout.trim().contains(expected),
        "unexpected rolling_mean() output: got '{}', expected to contain '{}'",
        stdout.trim(),
        expected
    );
}
