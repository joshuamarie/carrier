box::use(
    ./conversions[...]
)

#' Convert pounds to grams
#' @export
lb_to_g = function(x) convert_mass(x, "lb", "g")

#' Convert grams to pounds
#' @export
g_to_lb = function(x) convert_mass(x, "g", "lb")

#' Convert ounces to kilograms
#' @export
oz_to_kg = function(x) convert_mass(x, "oz", "kg")

#' Convert kilograms to ounces
#' @export
kg_to_oz = function(x) convert_mass(x, "kg", "oz")