# carrier

> 🚧️ This project is under active development. Expect breaking changes.🚧

A module manager for [`{box}`](https://klmr.me/box/) modules.

`carrier` lets `box` modules bundled as `.rmbx` files, install them, and make the modules much more easily distributed. This is similar to Python's `pip` or `npm`.

## Installation

Pre-built binaries for Linux, macOS, and Windows will be available on the [Releases](https://github.com/joshuamarie/carrier/releases) page once the project reaches a stable release.

To install the development version of `carrier` from GitHub, one requires [Rust](https://www.rust-lang.org/tools/install) (stable toolchain), particularly `rustc` and `cargo` on your system to compile it from source.

``` bash
cargo install --git https://github.com/joshuamarie/carrier
```

## Requirements

The idea for a distributable module is simple. Similar to Python, the usual structure of `box` modules ALWAYS has the metadata called `carrier.toml`, and analogue of `DESCRIPTION` of R packages or `pyproject.toml` of Python packages. Then, the `__init__.R` file serves as an entry point of the modules, kinda similar to how `NAMESPACE` from R works.  

Here's an example structure of the module: 

```
<some-dir-name>/
├── carrier.toml   
├── README.md
└── <module-name>/
    ├── __init__.r
    ├── mod.r
    ├── mod2.r
    └── <submod>/
        ├── __init__.r
        └── example.r
```

If you know the structure of Python packages, this feels familiar to you. 

## How it works

`carrier` has few commands to manage the modules. 

*Note: `<name-of-the-module>` is a placeholder. Apply a valid name. *

1.  Either initiate an R module by own, or use `carrier init <name-of-the-module>` command: 

    ``` bash
    carrier init <name-of-the-module>
    ```

2.  Bundle the module from the top of the directory with:

    ``` bash
    carrier bundle .
    ```

3.  Either install the module after bundling it:

    ``` bash
    carrier install <name-of-the-module>.tar.gz
    ```
    
    or install the module from a GitHub repo:

    ``` bash
    carrier install gh:username/<name-of-the-module>
    ```
    
    <!-- By default, it installs the module, locally, but you can install the module globally: -->

    <!-- ``` bash -->
    <!-- carrier install <name-of-the-module>.rmbx --global -->
    <!-- ``` -->

4.  Remove the installed module

    ``` bash
    carrier remove <name-of-the-module>
    ```

## Using installed modules

`carrier` syncs with `{box}` R package (this includes the syntax). The `box::use()` automatically resolves the path where the installed modules belong.

Here's an example: 

```r
# carrier install gh:joshuamarie/carrier/tree/main/convert-proj
box::use(cv = convert)
cv$mass$mass_conversion_table(1000)
```
