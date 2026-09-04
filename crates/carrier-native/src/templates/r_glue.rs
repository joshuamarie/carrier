pub const HOOK: &str = r#"#' @export
dlls = NULL

.on_load = function(ns) {
    lib_dir = box::file(".lib")
    abi = file.path(lib_dir, ".abi")
    if (file.exists(abi)) {
        built = readLines(abi, warn = FALSE)
        r_version = paste(R.version$major, strsplit(R.version$minor, ".", fixed = TRUE)[[1]][1], sep = ".")
        if (built[1] != R.version$platform) stop("Compiled shared binary is for a different platform, rebuild with `carrier compile`")
        if (built[2] != r_version) stop("Compiled shared binary is for a different R version, rebuild with `carrier compile`")
    }
    files = list.files(lib_dir, pattern = paste0(.Platform$dynlib.ext, "$"), full.names = TRUE)
    if (length(files) == 0) {
        stop("No compiled shared binary found in .lib/")
    }
    names(files) = tools::file_path_sans_ext(basename(files))
    ns$dll_paths = files
    ns$dlls = lapply(files, dyn.load)
}

.on_unload = function(ns) {
    for (path in ns$dll_paths) dyn.unload(path)
}
"#;

pub const HOOK_RCPP: &str = r#"box::use(Rcpp[...])

#' @export
dlls = NULL

.on_load = function(ns) {
    lib_dir = box::file(".lib")
    abi = file.path(lib_dir, ".abi")
    if (file.exists(abi)) {
        built = readLines(abi, warn = FALSE)
        r_version = paste(R.version$major, strsplit(R.version$minor, ".", fixed = TRUE)[[1]][1], sep = ".")
        if (built[1] != R.version$platform) stop("Compiled shared binary is for a different platform, rebuild with `carrier compile`")
        if (built[2] != r_version) stop("Compiled shared binary is for a different R version, rebuild with `carrier compile`")
    }
    files = list.files(lib_dir, pattern = paste0(.Platform$dynlib.ext, "$"), full.names = TRUE)
    if (length(files) == 0) {
        stop("No compiled shared binary found in .lib/")
    }
    names(files) = tools::file_path_sans_ext(basename(files))
    ns$dll_paths = files
    ns$dlls = lapply(files, dyn.load)
}

.on_unload = function(ns) {
    for (path in ns$dll_paths) dyn.unload(path)
}
"#;

pub const HELLO: &str = r#"box::use(./hook[dlls])

#' Calling native code example 1: Hello World!
#'
#' Template function that calls into the module's compiled code.
#'
#' @param name A single string.
#'
#' @return A string: "Hello from <language>, <name>!"
#'
#' @export
hello_world = function(name) {
    .Call(dlls${{module_name}}$hello_world, name)
}
"#;

pub const ADD: &str = r#"box::use(./hook[dlls])

#' Calling native code example 2: Add function
#'
#' Template function that calls into the module's compiled code.
#'
#' @param x First term.
#' @param y Second term.
#'
#' @return The sum of `x` and `y`, as a double.
#'
#' @export
add = function(x, y) {
    .Call(dlls${{module_name}}$add, x, y)
}
"#;

pub const INIT: &str = r#"#' @export
box::use(
    ./hello[hello_world],
    ./add[add], 
)
"#;

pub const HELLO_PURE: &str = r#"#' Example function 1: Hello World!
#'
#' @param name A single string.
#'
#' @return A string: "Hello from R, <name>!"
#'
#' @export
hello_world = function(name) {
    paste0("Hello from R, ", name, "!")
}
"#;

pub const ADD_PURE: &str = r#"#' Example function 2: Add function
#'
#' @param x First term.
#' @param y Second term.
#'
#' @return The sum of `x` and `y`.
#'
#' @export
add = function(x, y) {
    x + y
}
"#;
