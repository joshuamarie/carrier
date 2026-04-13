box::use(
    ./conversions[...]
)

#' Convert carats to grams
#' @export
ct_to_g = function(x) convert_mass(x, "ct", "g")

#' Convert grams to carats
#' @export
g_to_ct = function(x) convert_mass(x, "g", "ct")