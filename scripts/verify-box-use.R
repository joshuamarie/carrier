cat("about to call box::use()\n"); flush(stdout())
box::use(
    fp = fpurrr,
    fpurrr/map,
)

cat("box::use() returned\n"); flush(stdout())

cat("== fp$map$call(1:5, sqrt) ==\n"); flush(stdout())
print(fp$map$call(1:5, sqrt))

cat("\n== map$call(1:5, sqrt) (submodule imported directly) ==\n"); flush(stdout())
print(map$call(1:5, sqrt))

cat("\n== fp$map$call@dbl(1:5, sqrt) (typed dispatch) ==\n"); flush(stdout())
print(fp$map$call@dbl(1:5, sqrt))

stopifnot(identical(fp$map$call@dbl(1:5, sqrt), sqrt(1:5)))
cat("\n\nbox successfully imported the carrier-installed module.\n")