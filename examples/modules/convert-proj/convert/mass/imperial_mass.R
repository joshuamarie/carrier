box::use(
    ./conversions[...]
)

#' Convert ounces to grams
#' @export
oz_to_g = function(x) convert_mass(x, "oz", "g")

#' Convert grams to ounces
#' @export
g_to_oz = function(x) convert_mass(x, "g", "oz")

#' Convert pounds to kilograms
#' @export
lb_to_kg = function(x) convert_mass(x, "lb", "kg")

#' Convert kilograms to pounds
#' @export
kg_to_lb = function(x) convert_mass(x, "kg", "lb")

#' Convert pounds to ounces
#' @export
lb_to_oz = function(x) convert_mass(x, "lb", "oz")

#' Convert ounces to pounds
#' @export
oz_to_lb = function(x) convert_mass(x, "oz", "lb")

#' Convert stone to pounds
#' @export
st_to_lb = function(x) convert_mass(x, "st", "lb")

#' Convert pounds to stone
#' @export
lb_to_st = function(x) convert_mass(x, "lb", "st")

#' Convert US short tons to kilograms
#' @export
ton_us_to_kg = function(x) convert_mass(x, "ton_us", "kg")

#' Convert kilograms to US short tons
#' @export
kg_to_ton_us = function(x) convert_mass(x, "kg", "ton_us")