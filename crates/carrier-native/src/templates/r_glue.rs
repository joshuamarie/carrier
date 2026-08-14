pub const HOOK: &str = r#"#' @export
dll = NULL

.on_load = function(ns) {
    ns$dll = dyn.load(box::file(paste0("lib/{{native_dir}}", .Platform$dynlib.ext)))
}

.on_unload = function(ns) {
    dyn.unload(box::file(paste0("lib/{{native_dir}}", .Platform$dynlib.ext)))
}
"#;

pub const HOOK_RCPP: &str = r#"box::use(Rcpp[...])

#' @export
dll = NULL

.on_load = function(ns) {
    ns$dll = dyn.load(box::file(paste0("lib/{{native_dir}}", .Platform$dynlib.ext)))
}

.on_unload = function(ns) {
    dyn.unload(box::file(paste0("lib/{{native_dir}}", .Platform$dynlib.ext)))
}
"#;

pub const HELLO: &str = r#"box::use(./hook[dll])

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
    .Call(dll$hello_world, name)
}
"#;

pub const ADD: &str = r#"box::use(./hook[dll])

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
    .Call(dll$add, x, y)
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
