# Convert package

This is an example `{box}` module package managed by `{carrier}`, where it contains the collection of example codes featuring conversion units. 

## Installation

See the [{carrier} installation guide](https://joshuamarie.com/carrier/installation.html) for the installation details first. Then, for the meantime, install the patched forked `{box}` version:

``` r
# install.packages('pak')
pak::pak("joshuamarie/box@feature/carrier-module-support")
```

Then, install this package via following:

``` bash
carrier install gh:joshuamarie/carrier/tree/main/examples/modules/convert-proj
```

## Usage

This module IS an R package, not on a traditional CRAN-style package system, and it has to be attached through `box::use()` and has similar paradigm as Python's. 

It has several ways to import the `convert` module:

``` r
box::use(
    convert,
    cv = convert, 
    convert/mass,
    tmp = convert/temp,
    ...
)
```
