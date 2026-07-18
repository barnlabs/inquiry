use anyhow::{Result, bail};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Formula {
    pub id: &'static str,
    pub name: &'static str,
    pub expression: &'static str,
    pub description: &'static str,
    pub variables: &'static [&'static str],
    pub caveat: &'static str,
    /// Human-auditable definitions or derivations. These are references, not
    /// claims that the source endorses Inquiry.
    pub references: &'static [&'static str],
}

pub const FORMULAS: &[Formula] = &[
    Formula {
        id: "compound-interest",
        name: "Compound interest",
        expression: "A = P(1 + r/n)^(nt)",
        description: "Future value with periodic compounding.",
        variables: &[
            "P principal",
            "r annual decimal rate",
            "n compounds per year",
            "t years",
        ],
        caveat: "Excludes taxes, fees, and rate changes.",
        references: &["https://openstax.org/books/college-algebra-2e/pages/6-key-equations"],
    },
    Formula {
        id: "cagr",
        name: "Compound annual growth rate",
        expression: "CAGR = (Vf/Vi)^(1/t) - 1",
        description: "Smoothed annual growth between two values.",
        variables: &["Vi initial value", "Vf final value", "t years"],
        caveat: "This is the constant annual rate implied by the endpoints; it does not describe volatility or intermediate cash flows.",
        references: &[
            "https://openstax.org/books/college-algebra-2e/pages/6-1-exponential-functions",
        ],
    },
    Formula {
        id: "z-score",
        name: "Z-score",
        expression: "z = (x - μ) / σ",
        description: "Distance from a mean in standard-deviation units.",
        variables: &[
            "x observation",
            "μ population mean",
            "σ population standard deviation",
        ],
        caveat: "Interpretation depends on distribution shape and data quality.",
        references: &["https://www.itl.nist.gov/div898/handbook/eda/section3/eda35h.htm"],
    },
    Formula {
        id: "confidence-mean",
        name: "Confidence interval for a mean",
        expression: "x̄ ± z* × σ/√n",
        description: "Normal-approximation interval when population variance is known or the approximation is justified.",
        variables: &[
            "x̄ sample mean",
            "z* critical value",
            "σ standard deviation",
            "n sample size",
        ],
        caveat: "Use a t interval when σ is estimated and assumptions fit; account for sampling design.",
        references: &["https://www.itl.nist.gov/div898/handbook/prc/section1/prc14.htm"],
    },
    Formula {
        id: "quadratic",
        name: "Quadratic formula",
        expression: "x = (-b ± √(b² - 4ac)) / (2a)",
        description: "Roots of ax² + bx + c = 0.",
        variables: &[
            "a nonzero quadratic coefficient",
            "b linear coefficient",
            "c constant",
        ],
        caveat: "A negative discriminant produces complex roots.",
        references: &["https://openstax.org/books/college-algebra-2e/pages/2-key-equations"],
    },
    Formula {
        id: "risk-ratio",
        name: "Risk ratio",
        expression: "RR = [a/(a+b)] / [c/(c+d)]",
        description: "Ratio of outcome risk in exposed and unexposed groups.",
        variables: &[
            "a exposed cases",
            "b exposed non-cases",
            "c unexposed cases",
            "d unexposed non-cases",
        ],
        caveat: "Association is not causation; confounding and study design matter.",
        references: &[
            "https://www.cdc.gov/field-epi-manual/php/chapters/analyze-interpret-data.html",
        ],
    },
    Formula {
        id: "doubling-time",
        name: "Exponential doubling time",
        expression: "Td = ln(2) / r",
        description: "Time for exponential growth at continuous rate r to double.",
        variables: &["r continuous growth rate per time unit"],
        caveat: "Invalid when growth is not approximately exponential or r ≤ 0.",
        references: &[
            "https://openstax.org/books/calculus-volume-1/pages/6-8-exponential-growth-and-decay",
        ],
    },
    Formula {
        id: "haversine",
        name: "Haversine distance",
        expression: "d = 2R asin(√[sin²(Δφ/2)+cos φ1 cos φ2 sin²(Δλ/2)])",
        description: "Great-circle distance between coordinates on a sphere.",
        variables: &[
            "φ latitude in radians",
            "λ longitude in radians",
            "R sphere radius",
        ],
        caveat: "Earth is not a perfect sphere; use geodesic methods for precision work.",
        references: &[
            "https://www.movable-type.co.uk/scripts/latlong.html",
            "https://geodesy.noaa.gov/TOOLS/Inv_Fwd/Inv_Fwd.html",
        ],
    },
];

pub fn find(id_or_name: &str) -> Result<&'static Formula> {
    let needle = id_or_name.trim().to_lowercase();
    FORMULAS
        .iter()
        .find(|f| f.id == needle || f.name.to_lowercase() == needle)
        .ok_or_else(|| anyhow::anyhow!("unknown formula: {id_or_name}"))
}

pub fn cagr(initial: f64, final_value: f64, years: f64) -> Result<f64> {
    if !initial.is_finite() || !final_value.is_finite() || !years.is_finite() {
        bail!("CAGR inputs must be finite");
    }
    if initial <= 0.0 || final_value < 0.0 || years <= 0.0 {
        bail!("CAGR requires initial > 0, final >= 0, and years > 0");
    }
    Ok((final_value / initial).powf(1.0 / years) - 1.0)
}

pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> Result<f64> {
    if !lat1.is_finite() || !lon1.is_finite() || !lat2.is_finite() || !lon2.is_finite() {
        bail!("coordinates must be finite");
    }
    if !(-90.0..=90.0).contains(&lat1)
        || !(-90.0..=90.0).contains(&lat2)
        || !(-180.0..=180.0).contains(&lon1)
        || !(-180.0..=180.0).contains(&lon2)
    {
        bail!("coordinates are out of range");
    }
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = (lat2 - lat1).to_radians();
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    Ok(6371.0088 * 2.0 * a.sqrt().asin())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_cagr() {
        assert!((cagr(100.0, 121.0, 2.0).unwrap() - 0.1).abs() < 1e-12);
    }
    #[test]
    fn ny_london_distance() {
        let d = haversine_km(40.7128, -74.006, 51.5074, -0.1278).unwrap();
        assert!((d - 5570.0).abs() < 20.0);
    }
    #[test]
    fn rejects_non_finite_inputs() {
        assert!(cagr(f64::NAN, 121.0, 2.0).is_err());
        assert!(haversine_km(f64::INFINITY, 0.0, 0.0, 0.0).is_err());
    }

    #[test]
    fn every_formula_has_https_references() {
        for formula in FORMULAS {
            assert!(
                !formula.references.is_empty(),
                "{} has no references",
                formula.id
            );
            assert!(
                formula
                    .references
                    .iter()
                    .all(|reference| reference.starts_with("https://")),
                "{} has a non-HTTPS reference",
                formula.id
            );
        }
    }
}
