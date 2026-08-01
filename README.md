# carrier

[![Build](https://github.com/joshuamarie/carrier/actions/workflows/build.yml/badge.svg)](https://github.com/joshuamarie/carrier/actions/workflows/build.yml)
[![Release](https://img.shields.io/github/v/release/joshuamarie/carrier)](https://github.com/joshuamarie/carrier/releases)
[![License](https://img.shields.io/github/license/joshuamarie/carrier)](https://github.com/joshuamarie/carrier/blob/main/LICENSE.md)

A module manager for [`{box}`](https://klmr.me/box/) modules.

`carrier` is another package manager for R, built in Rust, exclusive for `{box}` modules. The tasks it handle involves bundling and installment. The entire purpose of `carrier` is to make the packaging for `{box}` modules possible and to be easily distributed. The interface of `carrier` is similar to Python's `pip`.

## Installation

`carrier` is a command line interface (CLI) tool built in Rust. Pre-built binaries for Linux, macOS, and Windows is available on the [Releases](https://github.com/joshuamarie/carrier/releases).

You can install `carrier` using the Shell installers. 

1.  On Linux / macOS:

    ``` bash
    curl -sSL https://raw.githubusercontent.com/joshuamarie/carrier/refs/heads/main/scripts/install.sh | bash
    ```

2.  On Windows: 

    ``` bash
    powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/joshuamarie/carrier/refs/heads/main/scripts/install.ps1 | iex"
    ```

To install the specific version, use that version's URL instead of `latest`: 

``` bash
curl -LsSf https://github.com/joshuamarie/carrier/releases/download/v0.1.0/carrier-installer.sh | bash
```

``` bash
powershell -ExecutionPolicy Bypass -c "irm https://github.com/joshuamarie/carrier/releases/download/v0.1.0/carrier-installer.ps1 | iex"
```

To install the development version of `carrier` from GitHub, one requires [Rust](https://www.rust-lang.org/tools/install) (stable toolchain), particularly `rustc` and `cargo` on your system to compile it from source.

``` bash
cargo install --git https://github.com/joshuamarie/carrier
```

Then install the particular main dependency `{box}`. In a meantime, kindly install the package from the forked repo, as the patches for `carrier` support are written down there:

``` r
# Install the package through GitHub
# install.packages('pak')
pak::pak("joshuamarie/box@carrier-module-support")
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

*Note: `<name-of-the-module>` is a placeholder. Apply a valid name.*

1.  Either initiate an R module with `carrier.toml` metadata file by own, or use `carrier init <name-of-the-module>` command: 

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
    carrier install gh:username/<path-of-the-module>
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

There are patches along the source code of `{box}`. This way, `carrier` syncs with `{box}` R package (this includes the syntax). The `box::use()` automatically resolves the path where the installed modules belong.

Here's an example: 

```r
# carrier install gh:joshuamarie/carrier/tree/main/convert-proj
box::use(cv = convert)
cv$mass$mass_conversion_table(1000)
```
