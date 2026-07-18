use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportLevel {
    Supported,
    Scoped,
    HandoffOnly,
    LocalImport,
    ExplicitAbstention,
    NotShipped,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityRecord {
    pub id: &'static str,
    pub capability: &'static str,
    pub support: SupportLevel,
    pub surface: &'static str,
    pub evidence_or_destination: &'static str,
    pub outbound_data: &'static str,
    pub limitation: &'static str,
    pub abstains_when: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageMatrix {
    pub schema_version: &'static str,
    pub reviewed_on: &'static str,
    pub universal_coverage_claimed: bool,
    pub capabilities: Vec<CapabilityRecord>,
}

pub fn matrix() -> CoverageMatrix {
    CoverageMatrix {
        schema_version: "inquiry.coverage/v1",
        reviewed_on: "2026-07-17",
        universal_coverage_claimed: false,
        capabilities: vec![
            CapabilityRecord {
                id: "current-office-us-president",
                capability: "Current U.S. president and numbered U.S. presidents",
                support: SupportLevel::Supported,
                surface: "research, macOS, MCP",
                evidence_or_destination: "Wikidata exact office statements plus current USAGov corroboration when available",
                outbound_data: "the public officeholder query",
                limitation: "Biographical summaries remain separately sourced and may be secondary or institution-controlled.",
                abstains_when: "the office or jurisdiction cannot be bound exactly",
            },
            CapabilityRecord {
                id: "current-office-uk-monarch",
                capability: "Current UK monarch",
                support: SupportLevel::Supported,
                surface: "research, macOS, MCP",
                evidence_or_destination: "Wikidata exact office statement, UK Parliament current-reign data, and The Royal Family profile",
                outbound_data: "the public officeholder query",
                limitation: "The institutional biography is not independent evidence; portrait display requires a separate accepted Commons rights record.",
                abstains_when: "the jurisdiction is missing or current-office evidence conflicts",
            },
            CapabilityRecord {
                id: "ambiguous-current-office",
                capability: "Unqualified current president, king, or monarch",
                support: SupportLevel::ExplicitAbstention,
                surface: "research, macOS, MCP",
                evidence_or_destination: "none",
                outbound_data: "none",
                limitation: "Inquiry will not guess a jurisdiction.",
                abstains_when: "always, until the user supplies a jurisdiction",
            },
            CapabilityRecord {
                id: "public-research",
                capability: "Scoped public research",
                support: SupportLevel::Scoped,
                surface: "research, macOS, MCP",
                evidence_or_destination: "the connector ledger in docs/data-sources.md",
                outbound_data: "the minimized query and connector-specific parameters after local privacy review",
                limitation: "Coverage follows installed connectors and is not universal search or a guarantee of truth.",
                abstains_when: "policy, privacy, exact-binding, licensing, or source-quality gates fail",
            },
            CapabilityRecord {
                id: "faa-airport-status",
                capability: "Current U.S. airport operations status",
                support: SupportLevel::Supported,
                surface: "airport-status CLI and MCP tool",
                evidence_or_destination: "FAA National Airspace System airport-events feed",
                outbound_data: "one three-letter airport identifier",
                limitation: "Reports listed airport-level FAA events, not an individual flight status and not navigation guidance.",
                abstains_when: "offline, the identifier is invalid, the response is stale or malformed, or the FAA endpoint rate-limits the request",
            },
            CapabilityRecord {
                id: "nasa-eonet-open-events",
                capability: "NASA EONET open natural-event snapshot",
                support: SupportLevel::Scoped,
                surface: "live-events CLI and macOS Live workspace",
                evidence_or_destination: "exact NASA EONET v3 open-events endpoint",
                outbound_data: "fixed status=open and limit=50 parameters; no user query or identifier. The native app separately discloses system-managed Apple Maps viewport requests before constructing its map.",
                limitation: "One provider-curated snapshot, not real-time monitoring, comprehensive coverage, official event extent, or independent verification. No polling occurs; MapKit has separate provider privacy terms.",
                abstains_when: "offline, permission is absent or mismatched, the response violates bounds/schema, the provider rate-limits the request, or timestamps are invalid",
            },
            CapabilityRecord {
                id: "airline-flight-status",
                capability: "Individual airline flight status",
                support: SupportLevel::HandoffOnly,
                surface: "flight-status CLI and MCP tool",
                evidence_or_destination: "official carrier flight-status page",
                outbound_data: "none until the user opens the carrier page and enters the identifier",
                limitation: "Inquiry does not retrieve, cache, or invent the flight state.",
                abstains_when: "the carrier is unsupported or the identifier does not match the selected carrier",
            },
            CapabilityRecord {
                id: "faa-aircraft-registration",
                capability: "Single U.S. aircraft registration lookup",
                support: SupportLevel::LocalImport,
                surface: "aircraft-lookup CLI",
                evidence_or_destination: "a user-downloaded FAA Releasable Aircraft Database archive",
                outbound_data: "none",
                limitation: "One N-number per invocation; owner names, addresses, serial numbers, Mode S codes, coordinates, and movement history are omitted.",
                abstains_when: "the archive schema is unrecognized, the N-number is invalid, or the record is absent",
            },
            CapabilityRecord {
                id: "package-tracking",
                capability: "Package tracking",
                support: SupportLevel::HandoffOnly,
                surface: "package-tracking CLI and MCP tool",
                evidence_or_destination: "official USPS, UPS, FedEx, or DHL tracking page",
                outbound_data: "none by default; an identifier is put in the official URL only after explicit deep-link opt-in",
                limitation: "Inquiry does not call credentialed carrier APIs, scrape tracking pages, or claim a delivery state.",
                abstains_when: "the carrier is unsupported or the identifier is malformed",
            },
            CapabilityRecord {
                id: "local-reference-tables",
                capability: "Reviewed periodic-element and common metric-thread reference tables",
                support: SupportLevel::Supported,
                surface: "research CLI, macOS, in-app HTML, RTF, and CSV",
                evidence_or_destination: "versioned local rows with IUPAC, NIST, PubChem, and ISO source pointers",
                outbound_data: "none",
                limitation: "The thread table is a coarse-thread identification aid, not a tolerance, fit, strength, torque, or safety specification. Periodic categories preserve the source dataset's broad grouping.",
                abstains_when: "the request needs a standard edition, tolerance class, material/grade, application-specific safety value, or facts outside the reviewed columns",
            },
            CapabilityRecord {
                id: "local-analysis",
                capability: "Calculations, conversions, graphs, timelines, reports, and exports",
                support: SupportLevel::Supported,
                surface: "CLI and MCP; selected flows in macOS",
                evidence_or_destination: "deterministic local code and supplied cited data",
                outbound_data: "none",
                limitation: "A rendered artifact does not independently verify user-supplied claims.",
                abstains_when: "validation or bounded-computation limits fail",
            },
            CapabilityRecord {
                id: "inquiry-study",
                capability: "Private local course-folder indexing and recall exports",
                support: SupportLevel::Scoped,
                surface: "macOS and CLI; MCP only after explicit environment opt-in",
                evidence_or_destination: "one user-selected local folder",
                outbound_data: "none in local surfaces; MCP returns selected excerpts to its host/model provider after opt-in",
                limitation: "It is not a whole-disk indexer and blocks flagged assessments, secrets, and embedded instructions from recall exports.",
                abstains_when: "the folder is too broad, authorization is missing, or material is unsafe to export",
            },
            CapabilityRecord {
                id: "chatgpt-direct-local-mcp",
                capability: "Direct local ChatGPT MCP",
                support: SupportLevel::NotShipped,
                surface: "none",
                evidence_or_destination: "future official remote MCP or approved secure tunnel only",
                outbound_data: "none",
                limitation: "The local stdio MCP server is not a direct ChatGPT connection.",
                abstains_when: "always until a separately reviewed remote path is implemented and approved",
            },
        ],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopedLookupGuard {
    PackageTracking,
    FlightStatus,
    AircraftTracking,
    AircraftRegistration,
}

impl ScopedLookupGuard {
    pub fn report_query(self) -> &'static str {
        match self {
            Self::PackageTracking => "Scoped package-tracking request (identifier omitted)",
            Self::FlightStatus => "Scoped flight-status request (identifier omitted)",
            Self::AircraftTracking => "Blocked live-aircraft tracking request (identifier omitted)",
            Self::AircraftRegistration => {
                "Scoped aircraft-registration request (identifier omitted)"
            }
        }
    }

    pub fn summary(self) -> &'static str {
        match self {
            Self::PackageTracking => {
                "Inquiry did not send the package identifier to general research connectors. Use the scoped package-tracking handoff with an explicitly selected carrier."
            }
            Self::FlightStatus => {
                "Inquiry did not send the flight identifier to general research connectors. Use the scoped flight-status handoff or FAA airport-status tool."
            }
            Self::AircraftTracking => {
                "Inquiry does not provide live aircraft location or movement history. No external connector was contacted."
            }
            Self::AircraftRegistration => {
                "Inquiry did not send the N-number to general research connectors. Use the local aircraft-lookup command with a user-downloaded FAA archive."
            }
        }
    }

    pub fn warning(self) -> &'static str {
        match self {
            Self::PackageTracking => {
                "Package identifiers can expose delivery activity. Inquiry requires an explicit carrier and uses an official carrier page; it never guesses or invents delivery state."
            }
            Self::FlightStatus => {
                "The carrier handoff returns no flight state. FAA airport status is airport-level only and is not navigation guidance."
            }
            Self::AircraftTracking => {
                "Inquiry blocks live tail-number tracking, person-aircraft association, bulk enumeration, and pattern-of-life research."
            }
            Self::AircraftRegistration => {
                "The local FAA lookup accepts one N-number and omits owner identity, address, serial number, Mode S code, coordinates, and movement history."
            }
        }
    }
}

pub fn scoped_lookup_guard(query: &str) -> Option<ScopedLookupGuard> {
    let normalized = query
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let has = |phrase: &str| format!(" {normalized} ").contains(&format!(" {phrase} "));
    let words = normalized.split_whitespace().collect::<Vec<_>>();
    let has_direct_movement_lookup_intent = words
        .iter()
        .any(|word| matches!(*word, "track" | "status" | "where"));
    let has_package_context = words.iter().any(|word| {
        matches!(
            *word,
            "package" | "parcel" | "shipment" | "usps" | "ups" | "fedex" | "dhl"
        )
    });
    let has_flight_context = words
        .iter()
        .any(|word| matches!(*word, "flight" | "airline"));
    let has_aircraft_context = words.iter().any(|word| {
        matches!(
            *word,
            "aircraft" | "airplane" | "plane" | "registration" | "registry"
        )
    }) || has("n number lookup");

    let has_n_number = find_n_number(&words).is_some();
    let has_aircraft_movement_intent = words.iter().any(|word| {
        matches!(
            *word,
            "live" | "track" | "tracking" | "location" | "history" | "status" | "where"
        )
    });
    if has_n_number && has_aircraft_movement_intent {
        return Some(ScopedLookupGuard::AircraftTracking);
    }
    if has_n_number && has_aircraft_context {
        return Some(ScopedLookupGuard::AircraftRegistration);
    }

    let has_strong_ups_identifier = words.iter().any(|word| {
        word.len() == 18
            && word.starts_with("1z")
            && word.chars().all(|value| value.is_ascii_alphanumeric())
    });
    let has_package_identifier = words.iter().any(|word| {
        (7..=40).contains(&word.len())
            && word.chars().all(|value| value.is_ascii_alphanumeric())
            && word.chars().any(|value| value.is_ascii_digit())
    }) || has_separated_package_identifier(&words);
    let has_personal_package_intent = [
        "track my package",
        "track this package",
        "where is my package",
        "where is this package",
        "my package status",
        "this package status",
    ]
    .iter()
    .any(|phrase| has(phrase));
    let has_tracking_number_context = has("tracking number") || has("tracking id");
    if has_personal_package_intent
        || (has_strong_ups_identifier
            && (has_direct_movement_lookup_intent
                || has_package_context
                || has_tracking_number_context))
        || (has_package_identifier && (has_package_context || has_tracking_number_context))
    {
        return Some(ScopedLookupGuard::PackageTracking);
    }

    let has_flight_identifier = find_flight_identifier(&words).is_some();
    if has_flight_identifier && (has_flight_context || has_direct_movement_lookup_intent) {
        return Some(ScopedLookupGuard::FlightStatus);
    }
    None
}

fn find_n_number(words: &[&str]) -> Option<String> {
    for (index, word) in words.iter().enumerate() {
        let upper = word.to_ascii_uppercase();
        let candidate = if upper == "N" {
            let Some(next) = words.get(index + 1) else {
                continue;
            };
            let suffix = next.to_ascii_uppercase();
            format!("N{suffix}")
        } else {
            upper
        };
        let Some(suffix) = candidate.strip_prefix('N') else {
            continue;
        };
        let first_letter = suffix.chars().position(|value| value.is_ascii_alphabetic());
        if (1..=5).contains(&suffix.len())
            && suffix
                .chars()
                .next()
                .is_some_and(|value| value.is_ascii_digit() && value != '0')
            && suffix.chars().all(|value| value.is_ascii_alphanumeric())
            && !suffix.chars().any(|value| matches!(value, 'I' | 'O'))
            && first_letter.is_none_or(|index| {
                suffix[index..]
                    .chars()
                    .all(|value| value.is_ascii_alphabetic())
                    && suffix.len() - index <= 2
            })
        {
            return Some(candidate);
        }
    }
    None
}

fn find_flight_identifier(words: &[&str]) -> Option<String> {
    for (index, word) in words.iter().enumerate() {
        let upper = word.to_ascii_uppercase();
        let mut candidates = vec![upper.clone()];
        if upper.len() == 2
            && let Some(next) = words.get(index + 1)
        {
            candidates.push(format!("{upper}{}", next.to_ascii_uppercase()));
        }
        for candidate in candidates {
            let bytes = candidate.as_bytes();
            if !(3..=7).contains(&bytes.len())
                || !bytes[..2].iter().all(u8::is_ascii_alphanumeric)
                || !bytes[..2].iter().any(u8::is_ascii_alphabetic)
            {
                continue;
            }
            let suffix = &candidate[2..];
            let digit_count = suffix
                .chars()
                .take_while(|value| value.is_ascii_digit())
                .count();
            let trailing = &suffix[digit_count..];
            if (1..=4).contains(&digit_count)
                && (trailing.is_empty()
                    || (trailing.len() == 1
                        && trailing.chars().all(|value| value.is_ascii_alphabetic())))
            {
                return Some(candidate);
            }
        }
    }
    None
}

fn has_separated_package_identifier(words: &[&str]) -> bool {
    for start in 0..words.len() {
        let first = words[start];
        if first.len() > 40
            || !first.chars().all(|value| value.is_ascii_alphanumeric())
            || !first.chars().any(|value| value.is_ascii_digit())
        {
            continue;
        }
        let mut compact = String::new();
        let mut digit_count = 0_usize;
        for (offset, word) in words[start..].iter().take(10).enumerate() {
            if !word.chars().all(|value| value.is_ascii_alphanumeric())
                || !word.chars().any(|value| value.is_ascii_digit())
                || (offset > 0 && word.len() > 6)
            {
                break;
            }
            compact.push_str(word);
            digit_count += word.chars().filter(char::is_ascii_digit).count();
            if compact.len() > 40 {
                break;
            }
            if (7..=40).contains(&compact.len()) && digit_count >= 4 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_identifiers_do_not_route_to_general_research() {
        assert_eq!(
            scoped_lookup_guard("track UPS 1Z999AA10123456784"),
            Some(ScopedLookupGuard::PackageTracking)
        );
        assert_eq!(
            scoped_lookup_guard("track UPS 1Z-999-AA10-1234-5678-4"),
            Some(ScopedLookupGuard::PackageTracking)
        );
        assert_eq!(
            scoped_lookup_guard("track UPS 1Z 999 AA10 1234 5678 4"),
            Some(ScopedLookupGuard::PackageTracking)
        );
        for query in [
            "track 1Z999AA10123456784",
            "my tracking number is 1Z999AA10123456784",
            "where is 1Z999AA10123456784",
            "UPS 1Z999AA10123456784",
        ] {
            assert_eq!(
                scoped_lookup_guard(query),
                Some(ScopedLookupGuard::PackageTracking),
                "{query}"
            );
        }
        assert_eq!(
            scoped_lookup_guard("flight status AA123"),
            Some(ScopedLookupGuard::FlightStatus)
        );
        for query in [
            "AA123 status",
            "where is flight AA123",
            "track AA123",
            "track AA-123",
        ] {
            assert_eq!(
                scoped_lookup_guard(query),
                Some(ScopedLookupGuard::FlightStatus),
                "{query}"
            );
        }
        assert_eq!(
            scoped_lookup_guard("track aircraft N12345 live"),
            Some(ScopedLookupGuard::AircraftTracking)
        );
        assert_eq!(
            scoped_lookup_guard("aircraft registration N12345"),
            Some(ScopedLookupGuard::AircraftRegistration)
        );
        assert_eq!(
            scoped_lookup_guard("aircraft registration N-12345"),
            Some(ScopedLookupGuard::AircraftRegistration)
        );
    }

    #[test]
    fn category_research_is_not_mistaken_for_a_lookup() {
        assert_eq!(
            scoped_lookup_guard("compare package tracking products"),
            None
        );
        assert_eq!(
            scoped_lookup_guard("research privacy risks in flight tracking"),
            None
        );
        assert_eq!(
            scoped_lookup_guard("compare UPS tracking APIs released in 2026"),
            None
        );
        assert_eq!(
            scoped_lookup_guard("where is my package"),
            Some(ScopedLookupGuard::PackageTracking)
        );
        for query in [
            "find AB123 datasheet",
            "find RTX5090",
            "ISBN9780131103627",
            "find CVE202612345",
            "locate model AB123",
            "part number 927489999",
        ] {
            assert_eq!(scoped_lookup_guard(query), None, "{query}");
        }
        assert_eq!(
            scoped_lookup_guard("track N12345"),
            Some(ScopedLookupGuard::AircraftTracking)
        );
    }
}
