use anyhow::{Result, anyhow, bail};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum PackageCarrier {
    Usps,
    Ups,
    Fedex,
    Dhl,
}

impl PackageCarrier {
    pub fn label(self) -> &'static str {
        match self {
            Self::Usps => "USPS",
            Self::Ups => "UPS",
            Self::Fedex => "FedEx",
            Self::Dhl => "DHL",
        }
    }

    pub fn official_page(self) -> &'static str {
        match self {
            Self::Usps => "https://tools.usps.com/go/TrackConfirmAction",
            Self::Ups => "https://www.ups.com/track?loc=en_US",
            Self::Fedex => "https://www.fedex.com/fedextrack/",
            Self::Dhl => "https://www.dhl.com/global-en/home/tracking.html",
        }
    }

    fn query_key(self) -> &'static str {
        match self {
            Self::Usps => "tLabels",
            Self::Ups => "tracknum",
            Self::Fedex => "trknbr",
            Self::Dhl => "tracking-id",
        }
    }
}

impl FromStr for PackageCarrier {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "usps" | "united-states-postal-service" => Ok(Self::Usps),
            "ups" => Ok(Self::Ups),
            "fedex" | "fed-ex" => Ok(Self::Fedex),
            "dhl" => Ok(Self::Dhl),
            _ => Err(anyhow!(
                "unsupported carrier; choose USPS, UPS, FedEx, or DHL explicitly"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PackageTrackingHandoff {
    pub schema_version: &'static str,
    pub carrier: &'static str,
    pub tracking_identifier_display: String,
    pub official_tracking_url: String,
    pub identifier_in_url: bool,
    pub status_retrieved: bool,
    pub network_used: bool,
    pub privacy_notice: &'static str,
    pub limitation: &'static str,
}

pub fn tracking_handoff(
    carrier: PackageCarrier,
    tracking_identifier: &str,
    include_identifier_in_url: bool,
) -> Result<PackageTrackingHandoff> {
    let identifier = normalize_tracking_identifier(tracking_identifier)?;
    let mut url = Url::parse(carrier.official_page())?;
    if include_identifier_in_url {
        url.query_pairs_mut()
            .append_pair(carrier.query_key(), &identifier);
    }
    validate_official_carrier_url(carrier, &url)?;
    Ok(PackageTrackingHandoff {
        schema_version: "inquiry.package-handoff/v1",
        carrier: carrier.label(),
        tracking_identifier_display: masked_identifier(&identifier),
        official_tracking_url: url.into(),
        identifier_in_url: include_identifier_in_url,
        status_retrieved: false,
        network_used: false,
        privacy_notice: if include_identifier_in_url {
            "Opening this official carrier URL sends the tracking identifier to that carrier and may retain it in browser history. Inquiry did not open the URL or contact the carrier."
        } else {
            "The tracking identifier is not present in the URL. Open the official carrier page yourself, review its privacy controls, and enter the identifier there."
        },
        limitation: "Inquiry did not retrieve or infer a delivery state. Carrier APIs require credentials or customer access, so this release uses an explicit official handoff instead of scraping or bypassing controls.",
    })
}

fn normalize_tracking_identifier(value: &str) -> Result<String> {
    let normalized = value
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && *character != '-')
        .collect::<String>()
        .to_ascii_uppercase();
    if !(7..=40).contains(&normalized.len()) {
        bail!("tracking identifier must contain 7 to 40 letters or digits");
    }
    if !normalized
        .chars()
        .all(|character| character.is_ascii_alphanumeric())
    {
        bail!("tracking identifier may contain only ASCII letters, digits, spaces, or hyphens");
    }
    Ok(normalized)
}

fn masked_identifier(value: &str) -> String {
    let visible = value.chars().rev().take(4).collect::<Vec<_>>();
    format!("••••{}", visible.into_iter().rev().collect::<String>())
}

fn validate_official_carrier_url(carrier: PackageCarrier, url: &Url) -> Result<()> {
    if url.scheme() != "https" {
        bail!("official tracking URL must use HTTPS");
    }
    let expected = Url::parse(carrier.official_page())?;
    if url.host_str() != expected.host_str() || url.path() != expected.path() {
        bail!("official tracking URL escaped the selected carrier destination");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_handoff_does_not_put_identifier_in_url() {
        let result =
            tracking_handoff(PackageCarrier::Ups, "1Z 999-AA10-1234-5678-4", false).unwrap();
        assert!(!result.official_tracking_url.contains("1Z999"));
        assert_eq!(result.tracking_identifier_display, "••••6784");
        assert!(!result.status_retrieved);
        assert!(!result.network_used);
    }

    #[test]
    fn explicit_deep_links_stay_on_exact_official_hosts() {
        for (carrier, host, key) in [
            (PackageCarrier::Usps, "tools.usps.com", "tLabels="),
            (PackageCarrier::Ups, "www.ups.com", "tracknum="),
            (PackageCarrier::Fedex, "www.fedex.com", "trknbr="),
            (PackageCarrier::Dhl, "www.dhl.com", "tracking-id="),
        ] {
            let result = tracking_handoff(carrier, "1234567890", true).unwrap();
            let url = Url::parse(&result.official_tracking_url).unwrap();
            assert_eq!(url.host_str(), Some(host));
            assert!(url.query().unwrap_or_default().contains(key));
            assert!(result.identifier_in_url);
        }
    }

    #[test]
    fn carrier_is_never_inferred_and_malformed_identifiers_fail() {
        assert!(<PackageCarrier as FromStr>::from_str("maybe-carrier").is_err());
        assert!(tracking_handoff(PackageCarrier::Usps, "../../secret", false).is_err());
        assert!(tracking_handoff(PackageCarrier::Usps, "123", false).is_err());
    }
}
