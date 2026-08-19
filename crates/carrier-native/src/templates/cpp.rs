pub const SRC_EXT: &str = "cpp";

pub mod rcpp {
    pub const HELLO: &str = r#"#include <Rcpp.h>
#include <string>

extern "C" SEXP hello_world(SEXP name) {
    std::string cpp_name = Rcpp::as<std::string>(name);
    std::string msg = "Hello from Rcpp, " + cpp_name + "!";
    return Rcpp::wrap(msg);
}
"#;

    pub const ADD: &str = r#"#include <Rcpp.h>

extern "C" SEXP add(SEXP x, SEXP y) {
    double sum = Rcpp::as<double>(x) + Rcpp::as<double>(y);
    return Rcpp::wrap(sum);
}
"#;

    pub const MAKEVARS: &str = r#"PKG_CXXFLAGS = $(shell "${R_HOME}/bin/Rscript" -e "cat(Rcpp:::CxxFlags())")
PKG_LIBS = $(shell "${R_HOME}/bin/Rscript" -e "cat(Rcpp:::LdFlags())")
"#;
}

pub mod cpp11 {
    pub const HELLO: &str = r#"#include <cpp11.hpp>
#include <string>

extern "C" SEXP hello_world(SEXP name) {
    try {
        std::string cpp_name = cpp11::as_cpp<std::string>(name);
        std::string msg = "Hello from cpp11, " + cpp_name + "!";
        return cpp11::as_sexp(msg);
    } catch (std::exception const &e) {
        Rf_error("%s", e.what());
    }
}
"#;

    pub const ADD: &str = r#"#include <cpp11.hpp>

extern "C" SEXP add(SEXP x, SEXP y) {
    double sum = cpp11::as_cpp<double>(x) + cpp11::as_cpp<double>(y);
    return cpp11::as_sexp(sum);
}
"#;

    pub const MAKEVARS: &str = r#"PKG_CPPFLAGS = -I"$(shell "${R_HOME}/bin/Rscript" -e "cat(system.file('include', package = 'cpp11'))")"
CXX_STD = CXX11
"#;
}
