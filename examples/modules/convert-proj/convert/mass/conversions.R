box::use(
    ./const[grams_const]
)

#' Convert a mass value from one unit to another.
#' 
#' @param value Numeric vector of values to convert.  
#' @param from Character string: source unit (see supported units above).
#' @param to Character string: target unit (see supported units above).
#' 
#' @return Numeric vector of converted values.
#'
#' @examples
#' convert_mass(1, "kg", "lb")
#' convert_mass(5, "oz", "g")
#' 
#' @export
convert_mass = function(value, from, to) {
    from = tolower(from)
    to = tolower(to)
 
    valid_units = names(grams_const)
 
    if (!from %in% valid_units)
        stop(sprintf("Unknown 'from' unit: '%s'. Valid units: %s",
            from, paste(valid_units, collapse = ", ")))
 
    if (!to %in% valid_units)
        stop(sprintf("Unknown 'to' unit: '%s'. Valid units: %s",
            to, paste(valid_units, collapse = ", ")))
 
    value_in_grams = value * grams_const[[from]]
    value_in_grams / grams_const[[to]]
}

#' Print conversion table for a mass value in all supported units.
#'
#' @param value  Numeric scalar to convert.
#' @param from   Unit of the input value (default "kg").
#'
#' @examples
#' mass_conversion_table(1, "kg")
#' 
#' @export
mass_conversion_table = function(value, from = "kg") {
    units = names(grams_const)
    results = sapply(units, function(u) convert_mass(value, from, u))
 
    cat(sprintf("\n  Mass conversion table for %g %s\n", value, from))
    cat("  ", strrep("-", 35), "\n", sep = "")
    for (u in units) {
        cat(sprintf("  %-10s : %g\n", u, results[[u]]))
    }
    cat("  ", strrep("-", 35), "\n\n", sep = "")
    invisible(results)
}

