use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversion {
    pub input_value: f64,
    pub input_unit: String,
    pub output_value: f64,
    pub output_unit: String,
    pub formula: String,
}

pub fn convert(value: f64, from: &str, to: &str) -> Result<Conversion> {
    if !value.is_finite() {
        bail!("input value must be finite");
    }
    let from = normalize(from);
    let to = normalize(to);
    let from_category = unit_category(&from).ok_or_else(|| anyhow!("unsupported unit: {from}"))?;
    let to_category = unit_category(&to).ok_or_else(|| anyhow!("unsupported unit: {to}"))?;
    if from_category != to_category {
        bail!("cannot convert {from} ({from_category}) to {to} ({to_category})");
    }
    if from_category == "temperature" {
        let kelvin = temperature_to_kelvin(value, &from)?;
        if kelvin < -1e-12 {
            bail!("temperature is below absolute zero");
        }
        let output = kelvin_to_temperature(kelvin.max(0.0), &to)?;
        let formula = match (from.as_str(), to.as_str()) {
            ("c", "f") => "°F = °C × 9/5 + 32",
            ("f", "c") => "°C = (°F − 32) × 5/9",
            ("c", "k") => "K = °C + 273.15",
            ("k", "c") => "°C = K − 273.15",
            ("f", "k") => "K = (°F − 32) × 5/9 + 273.15",
            ("k", "f") => "°F = (K − 273.15) × 9/5 + 32",
            _ => "identity",
        };
        return Ok(result(value, &from, output, &to, formula));
    }
    if from == to {
        return Ok(result(value, &from, value, &to, "identity"));
    }

    let (base, _) = to_base(value, &from)?;
    let (output, _) = from_base(base, &to)?;
    let formula = "result = value × source_factor ÷ target_factor";
    if !output.is_finite() {
        return Err(anyhow!("conversion produced a non-finite result"));
    }
    Ok(result(value, &from, output, &to, formula))
}

fn unit_category(unit: &str) -> Option<&'static str> {
    match unit {
        "c" | "f" | "k" => Some("temperature"),
        "m" | "km" | "mi" | "ft" | "in" => Some("length"),
        "kg" | "g" | "lb" | "oz" => Some("mass"),
        "l" | "ml" | "gal_us" | "gal_imp" => Some("volume"),
        "s" | "min" | "h" => Some("time"),
        _ => None,
    }
}

fn temperature_to_kelvin(value: f64, unit: &str) -> Result<f64> {
    match unit {
        "c" => Ok(value + 273.15),
        "f" => Ok((value - 32.0) * 5.0 / 9.0 + 273.15),
        "k" => Ok(value),
        _ => bail!("unsupported temperature unit: {unit}"),
    }
}

fn kelvin_to_temperature(value: f64, unit: &str) -> Result<f64> {
    match unit {
        "c" => Ok(value - 273.15),
        "f" => Ok((value - 273.15) * 9.0 / 5.0 + 32.0),
        "k" => Ok(value),
        _ => bail!("unsupported temperature unit: {unit}"),
    }
}

fn result(input: f64, from: &str, output: f64, to: &str, formula: &str) -> Conversion {
    Conversion {
        input_value: input,
        input_unit: from.into(),
        output_value: output,
        output_unit: to.into(),
        formula: formula.into(),
    }
}

fn normalize(unit: &str) -> String {
    match unit.trim().to_lowercase().replace('°', "").as_str() {
        "celsius" | "centigrade" => "c".into(),
        "fahrenheit" => "f".into(),
        "kelvin" => "k".into(),
        "meter" | "meters" | "metre" | "metres" => "m".into(),
        "kilometer" | "kilometers" | "kilometre" | "kilometres" => "km".into(),
        "mile" | "miles" => "mi".into(),
        "foot" | "feet" => "ft".into(),
        "inch" | "inches" => "in".into(),
        "kilogram" | "kilograms" => "kg".into(),
        "gram" | "grams" => "g".into(),
        "pound" | "pounds" | "lbs" => "lb".into(),
        "ounce" | "ounces" => "oz".into(),
        "liter" | "liters" | "litre" | "litres" => "l".into(),
        "milliliter" | "milliliters" | "millilitre" | "millilitres" => "ml".into(),
        "us gallon" | "us gallons" | "u.s. gallon" | "u.s. gallons" => "gal_us".into(),
        "imperial gallon" | "imperial gallons" | "uk gallon" | "uk gallons" => "gal_imp".into(),
        "second" | "seconds" | "sec" => "s".into(),
        "minute" | "minutes" | "mins" => "min".into(),
        "hour" | "hours" | "hrs" => "h".into(),
        other => other.into(),
    }
}

fn to_base(value: f64, unit: &str) -> Result<(f64, &'static str)> {
    let pair = match unit {
        "m" => (value, "length"),
        "km" => (value * 1000.0, "length"),
        "mi" => (value * 1609.344, "length"),
        "ft" => (value * 0.3048, "length"),
        "in" => (value * 0.0254, "length"),
        "kg" => (value, "mass"),
        "g" => (value / 1000.0, "mass"),
        "lb" => (value * 0.45359237, "mass"),
        "oz" => (value * 0.028349523125, "mass"),
        "l" => (value, "volume"),
        "ml" => (value / 1000.0, "volume"),
        "gal_us" => (value * 3.785411784, "volume"),
        "gal_imp" => (value * 4.54609, "volume"),
        "s" => (value, "time"),
        "min" => (value * 60.0, "time"),
        "h" => (value * 3600.0, "time"),
        _ => bail!("unsupported unit: {unit}"),
    };
    Ok(pair)
}

fn from_base(value: f64, unit: &str) -> Result<(f64, &'static str)> {
    let pair = match unit {
        "m" => (value, "length"),
        "km" => (value / 1000.0, "length"),
        "mi" => (value / 1609.344, "length"),
        "ft" => (value / 0.3048, "length"),
        "in" => (value / 0.0254, "length"),
        "kg" => (value, "mass"),
        "g" => (value * 1000.0, "mass"),
        "lb" => (value / 0.45359237, "mass"),
        "oz" => (value / 0.028349523125, "mass"),
        "l" => (value, "volume"),
        "ml" => (value * 1000.0, "volume"),
        "gal_us" => (value / 3.785411784, "volume"),
        "gal_imp" => (value / 4.54609, "volume"),
        "s" => (value, "time"),
        "min" => (value / 60.0, "time"),
        "h" => (value / 3600.0, "time"),
        _ => bail!("unsupported unit: {unit}"),
    };
    Ok(pair)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn miles_to_km() {
        assert!((convert(1.0, "mi", "km").unwrap().output_value - 1.609344).abs() < 1e-10);
    }
    #[test]
    fn freezing_point() {
        assert!((convert(32.0, "f", "c").unwrap().output_value).abs() < 1e-10);
    }
    #[test]
    fn rejects_dimension_mismatch() {
        assert!(convert(1.0, "kg", "m").is_err());
    }
    #[test]
    fn rejects_unknown_identity_and_non_finite_input() {
        assert!(convert(5.0, "banana", "banana").is_err());
        assert!(convert(f64::NAN, "m", "m").is_err());
    }
    #[test]
    fn supports_all_temperature_pairs() {
        assert!((convert(32.0, "f", "k").unwrap().output_value - 273.15).abs() < 1e-10);
        assert!((convert(273.15, "k", "f").unwrap().output_value - 32.0).abs() < 1e-10);
    }
    #[test]
    fn rejects_below_absolute_zero() {
        assert!(convert(-1.0, "k", "c").is_err());
        assert!(convert(-274.0, "c", "k").is_err());
    }
    #[test]
    fn gallon_system_must_be_explicit() {
        assert!(convert(1.0, "gallon", "l").is_err());
        assert!((convert(1.0, "US gallon", "l").unwrap().output_value - 3.785411784).abs() < 1e-12);
        assert!(
            (convert(1.0, "imperial gallon", "l").unwrap().output_value - 4.54609).abs() < 1e-12
        );
    }
}
