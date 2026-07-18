use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentKind {
    ExactCurrentOffice,
    NumberedOffice,
    RecentEventMedia,
    LiveEvents,
    PackageTracking,
    FlightStatus,
    AircraftRegistration,
    ReferenceTable,
    MarketPriceComparison,
    GeneralResearch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentResolution {
    pub kind: IntentKind,
    pub label: String,
    pub requested_outputs: Vec<String>,
    pub clarification: Option<String>,
    pub rationale: String,
}

pub fn resolve(query: &str) -> IntentResolution {
    let normalized = normalize(query);
    let has = |phrase: &str| contains_phrase(&normalized, phrase);
    let words = normalized.split_whitespace().collect::<Vec<_>>();
    let asks_for_media = words.iter().any(|word| {
        matches!(
            *word,
            "image" | "images" | "photo" | "photos" | "picture" | "pictures" | "portrait"
        )
    });
    let asks_for_recent = words.iter().any(|word| {
        matches!(
            *word,
            "recent" | "latest" | "current" | "today" | "yesterday" | "live" | "realtime"
        )
    }) || has("real time");
    let event_language = words.iter().any(|word| {
        matches!(
            *word,
            "attack"
                | "attacks"
                | "bombing"
                | "bombings"
                | "strike"
                | "strikes"
                | "war"
                | "conflict"
                | "explosion"
                | "earthquake"
                | "wildfire"
                | "hurricane"
                | "flood"
                | "eruption"
                | "disaster"
        )
    });

    if has("periodic table") || has("chemical elements") || has("element table") {
        return IntentResolution {
            kind: IntentKind::ReferenceTable,
            label: "Periodic table reference".into(),
            requested_outputs: vec!["interactive_table".into(), "print".into(), "export".into()],
            clarification: None,
            rationale: "A reviewed local reference table is a better fit than web-search excerpts."
                .into(),
        };
    }
    if (has("screw size") || has("screw sizes") || has("thread size") || has("thread sizes"))
        && words.iter().any(|word| {
            matches!(
                *word,
                "common" | "metric" | "imperial" | "standard" | "table" | "chart"
            )
        })
    {
        return IntentResolution {
            kind: IntentKind::ReferenceTable,
            label: "Common screw and thread-size reference".into(),
            requested_outputs: vec!["interactive_table".into(), "print".into(), "export".into()],
            clarification: None,
            rationale: "The query asks for bounded dimensional facts that belong in a searchable reference table with standard/source notes.".into(),
        };
    }
    if has("nasa eonet")
        || ((has("world map")
            || has("live events")
            || has("events in real time")
            || has("events realtime"))
            && asks_for_recent)
    {
        return IntentResolution {
            kind: IntentKind::LiveEvents,
            label: "Bounded natural-event snapshot".into(),
            requested_outputs: vec!["map".into(), "event_list".into(), "source_timestamps".into()],
            clarification: None,
            rationale: "The query asks for a timestamped provider snapshot. Inquiry performs one bounded request and does not claim continuous refresh or comprehensive coverage.".into(),
        };
    }
    if asks_for_media && asks_for_recent && event_language {
        return IntentResolution {
            kind: IntentKind::RecentEventMedia,
            label: "Recent event and rights-aware media".into(),
            requested_outputs: vec!["event_identity".into(), "image".into(), "timeline".into(), "sources".into()],
            clarification: None,
            rationale: "Time-sensitive event media requires event resolution, date/place binding, independent corroboration, and file-specific rights checks.".into(),
        };
    }
    if has("tracking number") || has("track my package") || has("where is my package") {
        return IntentResolution {
            kind: IntentKind::PackageTracking,
            label: "Package tracking handoff".into(),
            requested_outputs: vec!["official_carrier_handoff".into()],
            clarification: None,
            rationale: "Package identifiers must stay out of general research connectors.".into(),
        };
    }
    if has("flight status") || has("track flight") {
        return IntentResolution {
            kind: IntentKind::FlightStatus,
            label: "Flight-status lookup".into(),
            requested_outputs: vec!["official_carrier_status".into(), "source_timestamp".into()],
            clarification: None,
            rationale: "Individual flight state belongs in a scoped carrier/airport connector, never general search.".into(),
        };
    }
    if has("aircraft registration") || has("aircraft registry") || has("n number lookup") {
        return IntentResolution {
            kind: IntentKind::AircraftRegistration,
            label: "Aircraft registration lookup".into(),
            requested_outputs: vec!["local_registration_record".into()],
            clarification: None,
            rationale: "Aircraft registration is a bounded local-import capability with identity and movement fields omitted.".into(),
        };
    }
    let office = words.iter().any(|word| {
        matches!(
            *word,
            "president" | "potus" | "king" | "monarch" | "chancellor"
        )
    }) || has("prime minister");
    let ordinal = words.iter().any(|word| {
        let digits = word.trim_end_matches(|value: char| value.is_ascii_alphabetic());
        !digits.is_empty() && digits.chars().all(|value| value.is_ascii_digit())
    });
    if office && ordinal {
        return IntentResolution {
            kind: IntentKind::NumberedOffice,
            label: "Numbered officeholder identity".into(),
            requested_outputs: vec!["identity".into(), "biography".into(), "sources".into()],
            clarification: None,
            rationale:
                "The query requests an exact ordinal office statement, not fuzzy person search."
                    .into(),
        };
    }
    if office {
        return IntentResolution {
            kind: IntentKind::ExactCurrentOffice,
            label: "Current officeholder identity".into(),
            requested_outputs: vec!["identity".into(), "biography".into(), "portrait".into(), "sources".into()],
            clarification: None,
            rationale: "Current-office identity requires exact jurisdiction and same-run official corroboration where supported.".into(),
        };
    }
    if (has("compare price") || has("compare the price") || has("price of"))
        && words
            .iter()
            .any(|word| matches!(*word, "compare" | "versus" | "vs" | "to"))
    {
        return IntentResolution {
            kind: IntentKind::MarketPriceComparison,
            label: "Market price comparison".into(),
            requested_outputs: vec!["comparison_table".into(), "units".into(), "observation_dates".into()],
            clarification: Some("Inquiry does not yet have a reviewed general retail-price connector. Specify the exact product/grade, quantity and currency basis, locations, and date range; otherwise Inquiry will abstain instead of returning encyclopedia excerpts.".into()),
            rationale: "A valid price comparison needs matched products, units, currencies, locations, and observation dates.".into(),
        };
    }
    IntentResolution {
        kind: IntentKind::GeneralResearch,
        label: "General public research".into(),
        requested_outputs: if asks_for_media {
            vec!["findings".into(), "image".into(), "sources".into()]
        } else {
            vec!["findings".into(), "sources".into()]
        },
        clarification: None,
        rationale: "No narrower reviewed intent matched; connector eligibility still follows the capability matrix and privacy policy.".into(),
    }
}

fn normalize(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() {
                value
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_phrase(normalized: &str, phrase: &str) -> bool {
    let phrase = normalize(phrase);
    format!(" {normalized} ").contains(&format!(" {phrase} "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_product_intents_before_keyword_search() {
        assert_eq!(
            resolve("show me a picture of the recent US bombing").kind,
            IntentKind::RecentEventMedia
        );
        assert_eq!(
            resolve("show a live world map of events in real time").kind,
            IntentKind::LiveEvents
        );
        assert_eq!(resolve("NASA EONET").kind, IntentKind::LiveEvents);
        assert_eq!(
            resolve("show the periodic table and elements").kind,
            IntentKind::ReferenceTable
        );
        assert_eq!(
            resolve("common metric screw sizes table").kind,
            IntentKind::ReferenceTable
        );
    }

    #[test]
    fn price_comparison_requires_a_real_data_contract() {
        let resolution = resolve("Compare the price of tea in China to the US");
        assert_eq!(resolution.kind, IntentKind::MarketPriceComparison);
        assert!(resolution.clarification.is_some());
    }
}
