//! Integration tests for the `carrier` CLI binary itself. These spawn
//! the actual compiled executable via `assert_cmd` rather than calling
//! into carrier-core's library functions directly, so they catch bugs
//! that only show up in the clap wiring (flag names, kebab-case
//! conversion, exit codes, required-arg handling) which unit-level tests
//! against `commands::*::run()` can't see.
//!
//! Each test spawns its own subprocess, so env vars set via `.env(...)`
//! (e.g. CARRIER_LIB) are isolated per test with no risk of cross-test
//! interference (no mutex needed here), unlike the CARRIER_LIB-mutating
//! tests in carrier-core's own test suite.

use assert_cmd::Command;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

fn unique_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("carrier-cli-test-{label}-{n}-{}", std::process::id()))
}

struct Scratch(PathBuf);
impl Scratch {
    /// Creates the directory immediately.
    fn new(label: &str) -> Self {
        let dir = unique_dir(label);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    /// Reserves a unique path without creating it, for dirs the CLI
    /// itself is expected to create (e.g. `carrier init`'s target dir).
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

fn carrier_cmd() -> Command {
    let mut cmd = Command::cargo_bin("carrier").expect("carrier binary should be built by `cargo test`");
    // Backtraces are opt-in noise on stderr that depends on the
    // developer's shell environment (RUST_BACKTRACE) — strip it so
    // stderr assertions are deterministic across machines and CI.
    cmd.env_remove("RUST_BACKTRACE");
    cmd
}

// ---- --version / --help / no subcommand ----

#[test]
fn version_flag_prints_crate_version() {
    let assert = carrier_cmd().arg("--version").assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.trim(), format!("carrier {}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_flag_lists_all_subcommands() {
    let assert = carrier_cmd().arg("--help").assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    for subcommand in ["init", "bundle", "install", "remove"] {
        assert!(stdout.contains(subcommand), "--help output missing '{subcommand}':\n{stdout}");
    }
}

#[test]
fn no_subcommand_exits_nonzero_with_usage() {
    let assert = carrier_cmd().assert().failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    // Deliberately not asserting on the exact "Usage: carrier" text — the
    // binary name in that line varies by platform (carrier vs
    // carrier.exe) and build config, so pin to structural content that's
    // stable either way.
    assert!(stderr.contains("Commands:"), "stderr was:\n{stderr}");
    assert!(stderr.contains("install"), "stderr was:\n{stderr}");
}

// ---- init ----

#[test]
fn init_creates_expected_project_layout() {
    let target = Scratch::reserved("init-layout");

    carrier_cmd()
        .args(["init", "mymod", "--dir-name", target.path().to_str().unwrap()])
        .assert()
        .success();

    assert!(target.path().join("carrier.toml").is_file());
    assert!(target.path().join("README.md").is_file());
    assert!(target.path().join("mymod").join("__init__.R").is_file());
}

#[test]
fn init_dir_name_flag_is_wired_to_clap_correctly() {
    // Specifically checks the --dir-name flag (kebab-case on the CLI)
    // actually reaches InitArgs.dir_name (snake_case in Rust) — a wiring
    // bug here wouldn't be caught by testing commands::init::run() with
    // a hand-built InitArgs directly.
    let target = Scratch::reserved("dir-name-wiring");

    carrier_cmd()
        .args(["init", "somemod", "--dir-name", target.path().to_str().unwrap()])
        .assert()
        .success();

    assert!(target.path().is_dir());
    let contents = std::fs::read_to_string(target.path().join("carrier.toml")).unwrap();
    assert!(contents.contains("name = \"somemod\""));
}

#[test]
fn init_missing_name_arg_fails_with_usage_error() {
    carrier_cmd().arg("init").assert().failure();
}

// ---- bundle ----

#[test]
fn bundle_produces_tar_gz_by_default() {
    let cwd = Scratch::new("bundle-cwd");
    let project = cwd.path().join("mymod-proj");

    carrier_cmd()
        .args(["init", "mymod", "--dir-name", project.to_str().unwrap()])
        .assert()
        .success();

    carrier_cmd()
        .current_dir(cwd.path())
        .args(["bundle", project.to_str().unwrap()])
        .assert()
        .success();

    assert!(cwd.path().join("mymod_0.1.0.tar.gz").is_file());
}

#[test]
fn bundle_rmbx_flag_produces_rmbx_extension() {
    let cwd = Scratch::new("bundle-rmbx-cwd");
    let project = cwd.path().join("mymod-proj");

    carrier_cmd()
        .args(["init", "mymod", "--dir-name", project.to_str().unwrap()])
        .assert()
        .success();

    carrier_cmd()
        .current_dir(cwd.path())
        .args(["bundle", project.to_str().unwrap(), "--rmbx"])
        .assert()
        .success();

    assert!(cwd.path().join("mymod_0.1.0.rmbx").is_file());
    assert!(!cwd.path().join("mymod_0.1.0.tar.gz").exists());
}

// ---- install / remove ----

#[test]
fn install_then_remove_round_trip() {
    let project_root = Scratch::new("install-project-root");
    let project = project_root.path().join("mymod-proj");
    let lib = Scratch::reserved("install-lib");

    carrier_cmd()
        .args(["init", "mymod", "--dir-name", project.to_str().unwrap()])
        .assert()
        .success();

    carrier_cmd()
        .env("CARRIER_LIB", lib.path())
        .args(["install", project.to_str().unwrap()])
        .assert()
        .success();

    let module_dir = lib.path().join("mymod");
    assert!(module_dir.join("__init__.R").is_file());

    carrier_cmd()
        .env("CARRIER_LIB", lib.path())
        .args(["remove", "mymod", "--force"])
        .assert()
        .success();

    assert!(!module_dir.exists());
}

#[test]
fn install_on_nonexistent_source_fails_with_clear_error() {
    let bogus = unique_dir("install-bogus-source");

    let assert = carrier_cmd()
        .args(["install", bogus.to_str().unwrap()])
        .assert()
        .failure()
        .code(1);

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("Expected a directory, .tar.gz, .rmbx, or gh:username/repo"),
        "stderr was:\n{stderr}"
    );
}

#[test]
fn remove_nonexistent_module_fails_with_clear_error() {
    let lib = Scratch::new("remove-empty-lib");

    let assert = carrier_cmd()
        .env("CARRIER_LIB", lib.path())
        .args(["remove", "doesnotexist", "--force"])
        .assert()
        .failure()
        .code(1);

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(stderr.contains("is not installed"), "stderr was:\n{stderr}");
}

#[test]
fn remove_without_force_respects_declined_confirmation() {
    let project_root = Scratch::new("remove-confirm-project-root");
    let project = project_root.path().join("mymod-proj");
    let lib = Scratch::reserved("remove-confirm-lib");

    carrier_cmd()
        .args(["init", "mymod", "--dir-name", project.to_str().unwrap()])
        .assert()
        .success();

    carrier_cmd()
        .env("CARRIER_LIB", lib.path())
        .args(["install", project.to_str().unwrap()])
        .assert()
        .success();

    let module_dir = lib.path().join("mymod");
    assert!(module_dir.exists());

    // No --force: the CLI should prompt on stdin. Answering "n" must
    // decline the removal, leave the module installed, and still exit
    // successfully (matches ops::remove::run's "Aborted." path).
    let assert = carrier_cmd()
        .env("CARRIER_LIB", lib.path())
        .args(["remove", "mymod"])
        .write_stdin("n\n")
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Aborted."), "stdout was:\n{stdout}");

    assert!(module_dir.exists(), "module should still be installed after declining removal");
}

// ---- bare-name reservation (module registry conflict) ----
//
// A bare argument with no path separator and no leading `.` must never
// match a same-named local directory, even when one exists in the CWD.
// That name is reserved for a future module registry lookup, mirroring
// pip's `_looks_like_path` behavior (judged by appearance only, never by
// checking the filesystem). Local installs require an explicit signal:
// `./name`, `../name`, an absolute path, or a recognized archive
// extension.

#[test]
fn install_bare_name_matching_local_dir_is_reserved_not_silently_installed() {
    let cwd = Scratch::new("bare-name-cwd");
    let project = cwd.path().join("convert-proj");

    carrier_cmd()
        .args(["init", "convert", "--dir-name", project.to_str().unwrap()])
        .assert()
        .success();

    // Bare name, no ./ prefix, even though convert-proj/ genuinely exists
    // right here in cwd — must NOT silently install it.
    let assert = carrier_cmd()
        .current_dir(cwd.path())
        .args(["install", "convert-proj"])
        .assert()
        .failure()
        .code(1);

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(stderr.contains("looks like a module name"), "stderr was:\n{stderr}");
    assert!(stderr.contains("registry"), "stderr was:\n{stderr}");
}

#[test]
fn install_explicit_relative_path_still_installs_local_dir() {
    let cwd = Scratch::new("explicit-relative-cwd");
    let project = cwd.path().join("convert-proj");
    let lib = Scratch::reserved("explicit-relative-lib");

    carrier_cmd()
        .args(["init", "convert", "--dir-name", project.to_str().unwrap()])
        .assert()
        .success();

    // Same directory as above, but with an explicit ./ signal this time.
    carrier_cmd()
        .current_dir(cwd.path())
        .env("CARRIER_LIB", lib.path())
        .args(["install", "./convert-proj"])
        .assert()
        .success();

    assert!(lib.path().join("convert").join("__init__.R").is_file());
}

#[test]
fn install_bare_archive_filename_still_works_without_dot_slash() {
    let cwd = Scratch::new("bare-archive-cwd");
    let project = cwd.path().join("mymod-proj");
    let lib = Scratch::reserved("bare-archive-lib");

    carrier_cmd()
        .args(["init", "mymod", "--dir-name", project.to_str().unwrap()])
        .assert()
        .success();

    carrier_cmd()
        .current_dir(cwd.path())
        .args(["bundle", project.to_str().unwrap()])
        .assert()
        .success();

    // A bare .tar.gz filename (no separator, no leading dot) must still
    // work without needing ./ — the archive-extension escape hatch.
    carrier_cmd()
        .current_dir(cwd.path())
        .env("CARRIER_LIB", lib.path())
        .args(["install", "mymod_0.1.0.tar.gz"])
        .assert()
        .success();

    assert!(lib.path().join("mymod").join("__init__.R").is_file());
}

// ── --repo scaffolding (no registry backend yet) ─────────────────────
//
// The flag, arg threading, and mutual-exclusivity checks are real and
// tested here even though there's no registry protocol to actually talk
// to yet — install_from_registry's body is the one piece intentionally
// left as a stub.

#[test]
fn install_bare_name_with_repo_hits_the_not_implemented_stub() {
    let assert = carrier_cmd()
        .args(["install", "somepkg", "--repo", "https://modules.example.com"])
        .assert()
        .failure()
        .code(1);

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(stderr.contains("registries aren't implemented yet"), "stderr was:\n{stderr}");
    assert!(stderr.contains("somepkg"), "stderr was:\n{stderr}");
    assert!(stderr.contains("https://modules.example.com"), "stderr was:\n{stderr}");
}

#[test]
fn install_repo_flag_rejected_with_gh_source() {
    let assert = carrier_cmd()
        .args(["install", "gh:someuser/somerepo", "--repo", "https://modules.example.com"])
        .assert()
        .failure()
        .code(1);

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(stderr.contains("--repo doesn't apply to gh:"), "stderr was:\n{stderr}");
}

#[test]
fn install_repo_flag_rejected_with_local_path_source() {
    let cwd = Scratch::new("repo-flag-local-path-cwd");
    let project = cwd.path().join("mymod-proj");

    carrier_cmd()
        .args(["init", "mymod", "--dir-name", project.to_str().unwrap()])
        .assert()
        .success();

    let assert = carrier_cmd()
        .args(["install", "./mymod-proj", "--repo", "https://modules.example.com"])
        .current_dir(cwd.path())
        .assert()
        .failure()
        .code(1);

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(stderr.contains("--repo doesn't apply to local paths"), "stderr was:\n{stderr}");
}

#[test]
fn install_bare_name_without_repo_still_gets_the_original_reserved_error() {
    // Unchanged behavior from before --repo existed: no --repo means the
    // bare name is still just reserved, not resolvable to anything.
    let assert = carrier_cmd().args(["install", "somepkg"]).assert().failure().code(1);

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(stderr.contains("looks like a module name"), "stderr was:\n{stderr}");
    assert!(stderr.contains("--repo"), "stderr was:\n{stderr}");
}
