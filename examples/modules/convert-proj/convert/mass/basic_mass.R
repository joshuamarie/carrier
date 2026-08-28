box::use(
    ./conversions[...]
)

#' Convert milligrams to grams
#' @export
mg_to_g = function(x) convert_mass(x, "mg", "g")

#' Convert grams to milligrams
#' @export
g_to_mg = function(x) convert_mass(x, "g", "mg")

#' Convert grams to kilograms
#' @export
g_to_kg = function(x) convert_mass(x, "g", "kg")

#' Convert kilograms to grams
#' @export
kg_to_g = function(x) convert_mass(x, "kg", "g")

#' Convert kilograms to metric tonnes
#' @export
kg_to_t = function(x) convert_mass(x, "kg", "t")

#' Convert metric tonnes to kilograms
#' @export
t_to_kg = function(x) convert_mass(x, "t", "kg")