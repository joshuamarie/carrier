#include <R.h>
#include <Rinternals.h>
#include <math.h>

typedef struct {
    SEXP f;
    SEXP env;
} callback_data;

static double eval_f(double x, void *data) {
    callback_data *cd = (callback_data *) data;
    SEXP call = PROTECT(Rf_lang2(cd->f, Rf_ScalarReal(x)));
    SEXP out = PROTECT(Rf_eval(call, cd->env));
    double value = Rf_asReal(out);
    UNPROTECT(2);
    return value;
}

static double brentq_core(double (*f)(double, void *), void *data,
        double xa, double xb, double xtol, double rtol, int maxiter, int *converged) {
    double fa = f(xa, data);
    double fb = f(xb, data);
    *converged = 0;

    if (fa * fb > 0.0) {
        Rf_error("f(lower) and f(upper) must have opposite signs");
    }

    double xpre = xa, xcur = xb;
    double fpre = fa, fcur = fb;
    double xblk = 0.0, fblk = 0.0, spre = 0.0, scur = 0.0;

    for (int i = 0; i < maxiter; i++) {
        if (fpre != 0.0 && fcur != 0.0 && ((fpre < 0) != (fcur < 0))) {
            xblk = xpre;
            fblk = fpre;
            spre = scur = xcur - xpre;
        }
        if (fabs(fblk) < fabs(fcur)) {
            xpre = xcur; xcur = xblk; xblk = xpre;
            fpre = fcur; fcur = fblk; fblk = fpre;
        }

        double delta = (xtol + rtol * fabs(xcur)) / 2.0;
        double sbis = (xblk - xcur) / 2.0;

        if (fcur == 0.0 || fabs(sbis) < delta) {
            *converged = 1;
            return xcur;
        }

        if (fabs(spre) > delta && fabs(fcur) < fabs(fpre)) {
            double stry;
            if (xpre == xblk) {
                stry = -fcur * (xcur - xpre) / (fcur - fpre);
            } else {
                double dpre = (fpre - fcur) / (xpre - xcur);
                double dblk = (fblk - fcur) / (xblk - xcur);
                stry = -fcur * (fblk * dblk - fpre * dpre) / (dblk * dpre * (fblk - fpre));
            }
            if (2 * fabs(stry) < fmin(fabs(spre), 3 * fabs(sbis) - delta)) {
                spre = scur;
                scur = stry;
            } else {
                spre = sbis;
                scur = sbis;
            }
        } else {
            spre = sbis;
            scur = sbis;
        }

        xpre = xcur;
        fpre = fcur;
        if (fabs(scur) > delta) {
            xcur += scur;
        } else {
            xcur += (sbis > 0 ? delta : -delta);
        }
        fcur = f(xcur, data);
    }

    return xcur;
}

SEXP c_brentq(SEXP f_, SEXP env_, SEXP lower_, SEXP upper_, SEXP xtol_, SEXP rtol_, SEXP maxiter_) {
    callback_data cd;
    cd.f = f_;
    cd.env = env_;

    int converged;
    double root = brentq_core(eval_f, &cd,
            Rf_asReal(lower_), Rf_asReal(upper_),
            Rf_asReal(xtol_), Rf_asReal(rtol_),
            Rf_asInteger(maxiter_), &converged);

    SEXP out = PROTECT(Rf_allocVector(VECSXP, 2));
    SET_VECTOR_ELT(out, 0, Rf_ScalarReal(root));
    SET_VECTOR_ELT(out, 1, Rf_ScalarLogical(converged));

    SEXP names = PROTECT(Rf_allocVector(STRSXP, 2));
    SET_STRING_ELT(names, 0, Rf_mkChar("root"));
    SET_STRING_ELT(names, 1, Rf_mkChar("converged"));
    Rf_setAttrib(out, R_NamesSymbol, names);

    UNPROTECT(2);
    return out;
}
