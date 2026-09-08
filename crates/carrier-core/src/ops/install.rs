mod archive;
mod github;
mod native;

use anyhow::{bail, Result};
use std::path::PathBuf;

pub use github::GitHubFetcher;

use archive::{install_from_dir, install_from_registry, install_from_tar};
use github::{install_from_github, parse_github_source};

enum InstallSource {
    Tar(PathBuf),
    Dir(PathBuf),
    GitHub { user: String, repo: String, git_ref: Option<String>, subpath: Option<String> },
    Registry { name: String, repo: String },
}

pub fn run(source: &str, install_deps: bool, repo: Option<&str>) -> Result<()> {
    match parse_source(source, repo)? {
        InstallSource::Tar(path) => install_from_tar(&path, install_deps, None),
        InstallSource::Dir(path) => install_from_dir(&path, install_deps),
        InstallSource::GitHub { user, repo, git_ref, subpath } => {
            install_from_github(&user, &repo, git_ref.as_deref(), subpath.as_deref(), install_deps)
        }
        InstallSource::Registry { name, repo } => install_from_registry(&name, &repo, install_deps),
    }
}

/// Mirrors pip's `_looks_like_path` heuristic: judged purely by the
/// string's *appearance*, never by touching the filesystem. A bare word
/// with no separator and no leading `.` is never treated as local, so it
/// stays available for a module registry to claim later. Installing a
/// local directory always requires an explicit `./name`, `../name`, or
/// absolute path, the same way `pip install pkg` never silently matches
/// a same-named local folder.
fn looks_like_path(s: &str) -> bool {
    s.starts_with('.')
        || s.contains(std::path::MAIN_SEPARATOR)
        || (cfg!(windows) && s.contains('/'))
}

fn parse_source(s: &str, repo: Option<&str>) -> Result<InstallSource> {
    if let Some(rest) = s.strip_prefix("gh:") {
        if repo.is_some() {
            bail!("--repo doesn't apply to gh: sources, the repository is already given in the gh: path.");
        }
        let gh = parse_github_source(rest)?;
        return Ok(InstallSource::GitHub {
            user: gh.user,
            repo: gh.repo,
            git_ref: gh.git_ref,
            subpath: gh.subpath,
        });
    }

    let path = PathBuf::from(s);
    let has_archive_extension =
        matches!(path.extension().and_then(|e| e.to_str()), Some("gz"));

    if looks_like_path(s) || has_archive_extension {
        if repo.is_some() {
            bail!("--repo doesn't apply to local paths or archive files, '{}' already names a specific file or directory.", s);
        }
        if path.is_dir() {
            return Ok(InstallSource::Dir(path));
        }
        return match path.extension().and_then(|e| e.to_str()) {
            Some("gz") => Ok(InstallSource::Tar(path)),
            _ => bail!(
                "Expected a directory, .tar.gz, or gh:username/repo, got '{}'.",
                s
            ),
        };
    }

    // Bare name, no path signal, no gh: prefix: --repo is what turns this
    // into a registry lookup. Without it, the name is still reserved,
    // just not resolvable to anything yet.
    match repo {
        Some(repo_url) => Ok(InstallSource::Registry { name: s.to_owned(), repo: repo_url.to_owned() }),
        None => bail!(
            "'{}' looks like a module name. Use --repo <url> to install from a \
             registry, a local path (e.g. ./{}), or gh:user/repo instead.",
            s,
            s
        ),
    }
}
