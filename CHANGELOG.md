# Changelog

# Development version

-   Compiled code support: a module can now declare `[native]` in `carrier.toml` to ship C, C++, Rcpp, Rust, or Fortran code alongside its R source.

    -   `carrier install` automatically compiles a module's native code as part of installing it — no manual build step. Compiled artifacts are cached locally (`~/.carrier/native-cache/`), keyed by source contents, platform, and R version, so unchanged code isn't recompiled on every install.

    -   `carrier init <name> --native <ingredients>` scaffolds a compiled-code module from scratch. A starter source file, build configuration, and a small loader helper wired into `__init__.R`, so no module has to hand-write `dyn.load()`/platform-extension logic itself. `--native` takes a comma-separated set, e.g. `--native c`, `--native rcpp`, `--native c,fortran`:

        ``` bash
        carrier init stats_native --native rcpp
        ```

    -   Compiled code doesn't have to live under `src/` 
    
        -   `[native]` accepts a `path` override:

            ``` toml
            [native]
            path = "native/"
            build_deps = { Rcpp = "*" }
            ```
# v0.1.2

## Fixes

- Bundled `.rmbx`/`.tar.gz` archives now carry the resolved `carrier.lock` in their manifest. Previously, installing a standalone archive on a machine that never had the source project re-resolved dependencies fresh every time, silently ignoring any pin the project's lock had made.

- `carrier.toml`'s `[test]` config and extended author fields (`email`, `url`, `orcid`) were dropped when a bundled archive's metadata got reconstructed 

    -  `[test]` was never carried at all, and an extended author collapsed into a plain name string. Both now round-trip intact.

# v0.1.1

-   Add `--repo` support on `carrier install` for the future updates. 

-   A lockfile support on `carrier`. 

-   New lockfile module: read/write `carrier.lock`, pinning each package to an exact version and repo.

    -  Run `carrier lock .` to produce the `carrier.lock` lockfile. 

-   `module_deps` in `carrier.toml` can now declare a `source` alongside its version constraint, mirroring `package_deps`:

    ``` toml
    [module_deps]
    other_module = { version = "*", source = "gh:user/repo" }
    ```

-   Transitive module dependency resolution: a module's own `module_deps`/`package_deps` are now fetched and resolved recursively, not just the root project's. Dependency cycles are detected and rejected instead of silently resolving or hanging.

-   Fixes on `carrier install` command:

    -   `gh:user/repo/tree/<ref>/<subpath>` sources now install the pinned ref instead of silently falling back to the default branch.

    -   Previously accepts bare names to install the module under local dir (e.g. `carrier install some-dir`), now flags an error

        -   You have to place `.` prefix or `/` suffix if you want to install the module under some local dir, e.g. `carrier install ./some-dir` or `carrier install some-dir/`

-   Fixes on `carrier install` command: 
    
    -  `gh:user/repo/tree/<ref>/<subpath>` sources now install the pinned ref instead of silently falling back to the default branch.
    -  Previously accepts bare names to install the module under local dir (e.g. `carrier install some-dir`), now flags an error
       
       -  You have to place `.` prefix or `/` suffix if you want to install the module under some local dir, e.g. `carrier install ./some-dir` or `carrier install some-dir/`

# 0.1.0

-   This is the initial version release of `carrier`

-   Built the very first foundation for managing `{box}` modules as installable, distributable packages.

-   Here are the 4 commands:

    -   `carrier init` (Optional): Initializing the project, akin to `usethis::create_package()`
    -   `carrier bundle`: When sourcing the `{box}` modules
    -   `carrier install`: Install the package, analogous to `install.packages()`
    -   `carrier remove`: Remove the package
    