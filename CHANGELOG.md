# Changelog

# Development version

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

-   Fix: `gh:user/repo/tree/<ref>/<subpath>` sources now install the pinned ref instead of silently falling back to the default branch.

# 0.1.0

-   This is the initial version release of `carrier`

-   Built the very first foundation for managing `{box}` modules as installable, distributable packages.

-   Here are the 4 commands:

    -   `carrier init` (Optional): Initializing the project, akin to `usethis::create_package()`
    -   `carrier bundle`: When sourcing the `{box}` modules
    -   `carrier install`: Install the package, analogous to `install.packages()`
    -   `carrier remove`: Remove the package
    