box::use(./hook[dlls])

#' Fit a natural cubic spline
#'
#' Fits a piecewise cubic through a set of points, one cubic per
#' interval, stitched together so the curve and its first two
#' derivatives are continuous at every interior knot.
#'
#' @details
#' On interval \code{i}, between \code{x[i]} and \code{x[i+1]}, the
#' spline is:
#'
#' \deqn{S_i(x) = y_i + b_i (x - x_i) + c_i (x - x_i)^2 + d_i (x - x_i)^3}
#'
#' With \code{n} points there are \code{n - 1} intervals and
#' \code{4(n - 1)} unknown coefficients. These are pinned down by:
#'
#' \itemize{
#'   \item Interpolation: each cubic must hit its own two endpoints
#'     exactly, giving \eqn{2(n-1)} equations.
#'   \item First derivative continuity at every interior knot, so the
#'     slope does not jump: \eqn{S_i'(x_{i+1}) = S_{i+1}'(x_{i+1})},
#'     giving \eqn{n-2} equations.
#'   \item Second derivative continuity at every interior knot, so the
#'     curvature does not jump either:
#'     \eqn{S_i''(x_{i+1}) = S_{i+1}''(x_{i+1})}, giving \eqn{n-2}
#'     more equations.
#'   \item Two boundary conditions to close the system. This is a
#'     natural spline, meaning curvature is set to zero at both ends:
#'     \eqn{S_0''(x_0) = 0} and \eqn{S_{n-1}''(x_n) = 0}.
#' }
#'
#' Rather than solving for \eqn{b_i, c_i, d_i} directly, \eqn{c_i} is
#' treated as (half) the second derivative at each knot, and the other
#' two coefficients are recovered from it algebraically:
#'
#' \deqn{b_i = \frac{y_{i+1} - y_i}{h_i} - \frac{h_i (c_{i+1} + 2 c_i)}{3}}
#' \deqn{d_i = \frac{c_{i+1} - c_i}{3 h_i}}
#'
#' where \eqn{h_i = x_{i+1} - x_i}. Substituting the derivative
#' continuity conditions leaves one equation per interior knot,
#' involving only three neighboring unknowns:
#'
#' \deqn{h_{i-1} c_{i-1} + 2(h_{i-1} + h_i) c_i + h_i c_{i+1} =
#' 3 \left( \frac{y_{i+1} - y_i}{h_i} - \frac{y_i - y_{i-1}}{h_{i-1}} \right)}
#'
#' Stacked across all interior knots, this is a tridiagonal linear
#' system, nonzero only on the main diagonal and its two neighbors.
#' It is solved with the Thomas algorithm: one forward sweep that
#' eliminates the sub-diagonal, followed by one backward sweep that
#' substitutes to recover each \eqn{c_i}. This runs in \eqn{O(n)},
#' linear in the number of points, rather than the \eqn{O(n^3)} a
#' dense solve would cost.
#'
#' @param x Numeric vector of x-coordinates, strictly increasing.
#' @param y Numeric vector of y-coordinates, same length as \code{x}.
#' @return An object of class \code{rcalc_spline}, holding a pointer
#'   to the fitted spline. Pass it to \code{eval_spline()} to evaluate
#'   at new x-values.
#' @export
fit = function(x, y) {
    stopifnot(length(x) == length(y))
    ptr = .Call(getNativeSymbolInfo("cpp_spline_fit", PACKAGE = dlls$cpp), as.double(x), as.double(y))
    structure(list(ptr = ptr), class = "rcalc_spline")
}

#' Evaluate a fitted cubic spline
#'
#' Evaluates \eqn{S_i(x) = y_i + b_i (x - x_i) + c_i (x - x_i)^2 + d_i
#' (x - x_i)^3} at each query point, using the interval whose knots
#' bracket that point. Query points outside the fitted range are
#' evaluated by extending the nearest interval's cubic, which is not
#' the same as extrapolating linearly and can diverge quickly far
#' from the fitted range.
#'
#' @param spline An \code{rcalc_spline} object from \code{fit_spline()}.
#' @param query Numeric vector of x-values to evaluate at.
#' @return A numeric vector of interpolated y-values, same length as
#'   \code{query}.
#' 
#' @examples
#' box::use(rcalc/spline)
#' temp = spline$fit(
#'     c(35, 36, 39, 42, 45, 48), 
#'     c(
#'         2.87671519825595, 4.04868309245341, 3.95202175000174,   
#'         3.87683188946186, 4.07739945984612, 2.16064840967985
#'     )
#' )
#' spline$eval(temp, c(35, 36, 39, 42, 45, 48))
#' 
#' @export
eval = function(spline, query) {
    stopifnot(inherits(spline, "rcalc_spline"))
    .Call(getNativeSymbolInfo("cpp_spline_eval", PACKAGE = dlls$cpp), spline$ptr, as.double(query))
}
