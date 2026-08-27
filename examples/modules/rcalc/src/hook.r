box::use(Rcpp[...])

#' @export
dlls = NULL

.on_load = function(ns) {
    lib_dir = box::file(".lib")
    files = list.files(lib_dir, pattern = paste0(.Platform$dynlib.ext, "$"), full.names = TRUE)
    if (length(files) == 0) {
        stop("No compiled shared objects found in .lib/")
    }
    names(files) = tools::file_path_sans_ext(basename(files))
    ns$dll_paths = files
    ns$dlls = lapply(files, dyn.load)
}

.on_unload = function(ns) {
    for (path in ns$dll_paths) dyn.unload(path)
}
