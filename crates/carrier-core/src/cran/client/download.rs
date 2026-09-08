use std::io::Write as _;
use std::path::Path;

use anyhow::{Context, Result};

use crate::cran::binary_install;
use crate::paths::{detect_r_platform, RPlatformOs};

/// Download `{repo}/src/contrib/{pkg}_{ver}.tar.gz` and extract it
/// directly into `lib_path`, so `lib_path/{pkg}/` is the result.
pub(super) fn download_and_unpack(
    pkg: &str,
    version: &str,
    repo_url: &str,
    lib_path: &Path,
) -> Result<()> {
    let platform = detect_r_platform();
    if let Ok(platform) = &platform {
        if let Some(binary_url) = binary_url_for(pkg, version, repo_url, platform) {
            match try_install_binary(pkg, &binary_url, lib_path, &platform.arch) {
                Ok(()) => {
                    println!(" [binary] {} {} (no compilation needed)", pkg, version);
                    return Ok(());
                }
                Err(e) => {
                    eprintln!(" [warn] binary install failed for {} ({}), falling back to source...", pkg, e);
                }
            }
        }
    }

    let primary_url = format!(
        "{}/src/contrib/{}_{}.tar.gz",
        repo_url.trim_end_matches('/'),
        pkg,
        version
    );

    let response = reqwest::blocking::get(&primary_url)
        .with_context(|| format!("Failed to download {}", primary_url))?;

    let response = if response.status() == reqwest::StatusCode::NOT_FOUND {
        let archive_url = format!(
            "{}/src/contrib/Archive/{}/{}_{}.tar.gz",
            repo_url.trim_end_matches('/'),
            pkg,
            pkg,
            version
        );
        eprintln!(" [warn] {} not in src/contrib, trying archive...", pkg);
        let archive_resp = reqwest::blocking::get(&archive_url)
            .with_context(|| format!("Failed to download {}", archive_url))?;

        if archive_resp.status() == reqwest::StatusCode::NOT_FOUND {
            let dev_url = format!(
                "{}/src/contrib/{}_{}.9000.tar.gz",
                repo_url.trim_end_matches('/'),
                pkg,
                version
            );
            eprintln!(" [warn] {} not in archive, trying dev version...", pkg);
            reqwest::blocking::get(&dev_url)
                .with_context(|| format!("Failed to download {}", dev_url))?
        } else {
            archive_resp
        }
    } else {
        response
    };

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} downloading {} {}", response.status(), pkg, version);
    }

    let bytes = response.bytes()
        .with_context(|| format!("Failed to read bytes for {}", pkg))?;

    let mut tmp = tempfile::Builder::new()
        .suffix(".tar.gz")
        .tempfile()
        .with_context(|| format!("Failed to create temp file for {}", pkg))?;
    tmp.write_all(&bytes)
        .with_context(|| format!("Failed to write temp tarball for {}", pkg))?;
    let tmp_path = tmp.path();

    std::fs::create_dir_all(lib_path)
        .with_context(|| format!("Failed to create lib dir: {}", lib_path.display()))?;

    let lib_arg = format!(
        "--library={}",
        lib_path.to_str().context("lib_path contains invalid UTF-8")?
    );

    let status = std::process::Command::new("R")
        .args(["CMD", "INSTALL", "--no-multiarch", "--no-docs", "--no-help", &lib_arg])
        .arg(tmp_path)
        .status()
        .with_context(|| format!("Failed to run R CMD INSTALL for {} — is R on PATH?", pkg))?;

    if !status.success() {
        anyhow::bail!("R CMD INSTALL failed for {} {} (exit: {})", pkg, version, status);
    }

    Ok(())
}

fn binary_url_for(pkg: &str, version: &str, repo_url: &str, platform: &crate::paths::RPlatform) -> Option<String> {
    let base = repo_url.trim_end_matches('/');
    match &platform.os {
        RPlatformOs::Windows => Some(format!(
            "{}/bin/windows/contrib/{}/{}_{}.zip",
            base, platform.r_version_short, pkg, version
        )),
        RPlatformOs::MacOs => {
            // CRAN split macOS binaries by CPU architecture years ago.
            // bin/macosx/contrib (no arch) is a legacy path that some
            // mirrors still serve, populated with x86_64-only builds. Using
            // it unconditionally installs an Intel binary on an Apple
            // Silicon machine, which fails at dlopen() time, not at
            // install time. R.version$arch is "aarch64" on Apple Silicon
            // and "x86_64" on Intel; CRAN's own directory names are
            // "arm64" and "x86_64" respectively.
            let macos_arch = match platform.arch.as_str() {
                "aarch64" | "arm64" => "big-sur-arm64",
                "x86_64" => "big-sur-x86_64",
                other => {
                    eprintln!(
                        " [warn] unrecognized macOS architecture '{}', skipping binary install for {}",
                        other, pkg
                    );
                    return None;
                }
            };
            Some(format!(
                "{}/bin/macosx/{}/contrib/{}/{}_{}.tgz",
                base, macos_arch, platform.r_version_short, pkg, version
            ))
        }
        RPlatformOs::Linux(Some(codename)) => Some(format!(
            "{}/bin/linux/{}-{}/{}/src/contrib/{}_{}.tar.gz",
            base, codename, platform.arch, platform.r_version_short, pkg, version
        )),
        RPlatformOs::Linux(None) | RPlatformOs::Other => None,
    }
}

fn try_install_binary(pkg: &str, url: &str, lib_path: &Path, expected_arch: &str) -> Result<()> {
    let response = reqwest::blocking::get(url)
        .with_context(|| format!("Failed to download binary: {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} downloading binary {}", response.status(), url);
    }

    let bytes = response.bytes()
        .with_context(|| format!("Failed to read binary bytes for {}", pkg))?;

    let is_zip = bytes.starts_with(b"PK");
    let is_gzip = bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b;

    if !is_zip && !is_gzip {
        anyhow::bail!(
            "Response for {} was not a valid archive (likely no binary available for this version)",
            pkg
        );
    }

    let ext = if is_zip { "zip" } else { "tgz" };
    let mut tmp = tempfile::Builder::new()
        .suffix(&format!(".{}", ext))
        .tempfile()
        .with_context(|| format!("Failed to create temp file for {}", pkg))?;
    tmp.write_all(&bytes)
        .with_context(|| format!("Failed to write temp archive for {}", pkg))?;

    binary_install::install_binary_package(tmp.path(), lib_path, pkg, expected_arch)
}
