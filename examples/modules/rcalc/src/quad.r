#' @export
box::use(./hook[dlls])

#' @export
quad = function(f, lower, upper, tol = 1e-8, max_depth = 50) {
    stopifnot(is.function(f))
    sym = getNativeSymbolInfo("cpp_quad", PACKAGE = dlls$cpp)  # sym = dlls$cpp$cpp_quad
    .Call(sym, f, as.double(lower), as.double(upper), as.double(tol), as.integer(max_depth))
}
