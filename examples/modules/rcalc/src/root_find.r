#' @export
box::use(./hook[dlls])

#' @export
brentq = function(f, lower, upper, xtol = 1e-12, rtol = 1e-15, maxiter = 100) {
    stopifnot(is.function(f))
    sym = getNativeSymbolInfo("c_brentq", PACKAGE = dlls$c)
    .Call(sym, f, environment(), as.double(lower), as.double(upper),
          as.double(xtol), as.double(rtol), as.integer(maxiter))
}
