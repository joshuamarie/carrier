#include <Rcpp.h>
#include <R.h>
#include <Rinternals.h>

using Rcpp::Function;
using Rcpp::as; 

static double eval_f(Function f, double x, int &count) {
    count++;
    return as<double>(f(x));
}

static double simpson(double fa, double fm, double fb, double a, double b) {
    return (b - a) / 6.0 * (fa + 4.0 * fm + fb);
}

static double adaptive_simpson(
    Function f, 
    double a, double b,
    double fa, double fm, double fb, 
    double whole, 
    double tol,
    int depth, 
    int &count, 
    int max_depth
) {
    double m = (a + b) / 2.0;
    double lm = (a + m) / 2.0;
    double rm = (m + b) / 2.0;
    double flm = eval_f(f, lm, count);
    double frm = eval_f(f, rm, count);
    double left = simpson(fa, flm, fm, a, m);
    double right = simpson(fm, frm, fb, m, b);

    if (depth >= max_depth) {
        return left + right;
    }
    if (std::abs(left + right - whole) <= 15 * tol) {
        return left + right + (left + right - whole) / 15.0;
    }
    return adaptive_simpson(f, a, m, fa, flm, fm, left, tol / 2.0, depth + 1, count, max_depth) +
           adaptive_simpson(f, m, b, fm, frm, fb, right, tol / 2.0, depth + 1, count, max_depth);
}

extern "C" SEXP cpp_quad(SEXP f_, SEXP lower_, SEXP upper_, SEXP tol_, SEXP max_depth_) {
    try {
        Function f(f_);
        double lower = Rf_asReal(lower_);
        double upper = Rf_asReal(upper_);
        double tol = Rf_asReal(tol_);
        int max_depth = Rf_asInteger(max_depth_);

        int count = 0;
        double fa = eval_f(f, lower, count);
        double fb = eval_f(f, upper, count);
        double m = (lower + upper) / 2.0;
        double fm = eval_f(f, m, count);
        double whole = simpson(fa, fm, fb, lower, upper);

        double value = adaptive_simpson(f, lower, upper, fa, fm, fb, whole, tol, 0, count, max_depth);

        SEXP out = PROTECT(Rf_allocVector(VECSXP, 2));
        SET_VECTOR_ELT(out, 0, Rf_ScalarReal(value));
        SET_VECTOR_ELT(out, 1, Rf_ScalarInteger(count));

        SEXP names = PROTECT(Rf_allocVector(STRSXP, 2));
        SET_STRING_ELT(names, 0, Rf_mkChar("value"));
        SET_STRING_ELT(names, 1, Rf_mkChar("evaluations"));
        Rf_setAttrib(out, R_NamesSymbol, names);

        UNPROTECT(2);
        return out;
    } catch (std::exception &e) {
        Rf_error("%s", e.what());
    } catch (...) {
        Rf_error("unknown error in cpp_quad");
    }
    return R_NilValue;
}
