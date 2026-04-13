#' Convert Celsius to Fahrenheit
#' @export
celsius_to_fahrenheit = function(c) (c * 9/5) + 32

#' Convert Fahrenheit to Celsius
#' @export
fahrenheit_to_celsius = function(f) (f - 32) * 5/9

#' Convert Celsius to Kelvin
#' @export
celsius_to_kelvin = function(c) c + 273.15

#' Convert Kelvin to Celsius
#' @export
kelvin_to_celsius = function(k) k - 273.15

#' Convert Fahrenheit to Kelvin
#' @export
fahrenheit_to_kelvin = function(f) celsius_to_kelvin(fahrenheit_to_celsius(f))

#' Convert Kelvin to Fahrenheit
#' @export
kelvin_to_fahrenheit = function(k) celsius_to_fahrenheit(kelvin_to_celsius(k))
