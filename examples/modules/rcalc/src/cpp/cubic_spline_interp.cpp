#include <Rcpp.h>
#include <R.h>
#include <Rinternals.h>
#include <vector>
#include <stdexcept>
#include <algorithm>

class CubicSpline {
public:
    CubicSpline(std::vector<double> x, std::vector<double> y) : x_(x), y_(y) {
        int np = (int) x_.size();
        if (np < 3) {
            throw std::invalid_argument("cubic spline needs at least 3 points");
        }
        for (int i = 1; i < np; i++) {
            if (x_[i] <= x_[i - 1]) {
                throw std::invalid_argument("x must be strictly increasing");
            }
        }
        solve();
    }

    double eval(double query) const {
        int np = (int) x_.size();
        int n = np - 1;
        int j = (int) (std::upper_bound(x_.begin(), x_.end(), query) - x_.begin()) - 1;
        if (j < 0) j = 0;
        if (j > n - 1) j = n - 1;
        double dx = query - x_[j];
        return y_[j] + b_[j] * dx + c_[j] * dx * dx + d_[j] * dx * dx * dx;
    }

private:
    std::vector<double> x_, y_;
    std::vector<double> b_, c_, d_;

    void solve() {
        int np = (int) x_.size();
        int n = np - 1;

        std::vector<double> h(n);
        for (int i = 0; i < n; i++) {
            h[i] = x_[i + 1] - x_[i];
        }

        std::vector<double> alpha(n, 0.0);
        for (int i = 1; i < n; i++) {
            alpha[i] = 3.0 / h[i] * (y_[i + 1] - y_[i]) - 3.0 / h[i - 1] * (y_[i] - y_[i - 1]);
        }

        std::vector<double> l(np, 1.0), mu(np, 0.0), z(np, 0.0);
        for (int i = 1; i < n; i++) {
            l[i] = 2.0 * (x_[i + 1] - x_[i - 1]) - h[i - 1] * mu[i - 1];
            mu[i] = h[i] / l[i];
            z[i] = (alpha[i] - h[i - 1] * z[i - 1]) / l[i];
        }

        std::vector<double> c(np, 0.0);
        b_.assign(n, 0.0);
        d_.assign(n, 0.0);

        for (int j = n - 1; j >= 0; j--) {
            c[j] = z[j] - mu[j] * c[j + 1];
            b_[j] = (y_[j + 1] - y_[j]) / h[j] - h[j] * (c[j + 1] + 2.0 * c[j]) / 3.0;
            d_[j] = (c[j + 1] - c[j]) / (3.0 * h[j]);
        }

        c_.assign(c.begin(), c.end() - 1);
    }
};

static void spline_finalizer(SEXP ptr) {
    CubicSpline *spline = (CubicSpline *) R_ExternalPtrAddr(ptr);
    if (spline != nullptr) {
        delete spline;
        R_ClearExternalPtr(ptr);
    }
} 

extern "C" SEXP cpp_spline_fit(SEXP x_, SEXP y_) {
    try {
        int n = LENGTH(x_);
        std::vector<double> x(REAL(x_), REAL(x_) + n);
        std::vector<double> y(REAL(y_), REAL(y_) + n);

        CubicSpline *spline = new CubicSpline(x, y);
        SEXP ptr = PROTECT(R_MakeExternalPtr(spline, R_NilValue, R_NilValue));
        R_RegisterCFinalizerEx(ptr, spline_finalizer, TRUE);
        UNPROTECT(1);
        return ptr;
    } catch (std::exception &e) {
        Rf_error("%s", e.what());
    }
    return R_NilValue;
} 

extern "C" SEXP cpp_spline_eval(SEXP ptr_, SEXP query_) {
    try {
        CubicSpline *spline = (CubicSpline *) R_ExternalPtrAddr(ptr_);
        if (spline == nullptr) {
            Rf_error("spline object has been freed, fit a new one before evaluating");
        }

        int n = LENGTH(query_);
        double *pq = REAL(query_);
        SEXP out = PROTECT(Rf_allocVector(REALSXP, n));
        double *po = REAL(out);
        for (int i = 0; i < n; i++) {
            po[i] = spline->eval(pq[i]);
        }
        UNPROTECT(1);
        return out;
    } catch (std::exception &e) {
        Rf_error("%s", e.what());
    }
    return R_NilValue;
}
