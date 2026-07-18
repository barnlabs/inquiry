use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityLevel {
    None,
    Sensitive,
    HighlySensitive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyAssessment {
    pub level: SensitivityLevel,
    /// Category names only. Detected values are deliberately never returned.
    pub indicators: Vec<String>,
    pub requires_network_confirmation: bool,
    pub redacted_query: String,
    pub redaction_count: usize,
    pub redacted_query_safe_to_send: bool,
    pub guidance: String,
}

pub fn assess(query: &str) -> PrivacyAssessment {
    let mut redacted = query.to_owned();
    let mut indicators = Vec::new();
    let mut redaction_count = 0;

    for (label, pattern, replacement) in [
        (
            "email address",
            r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b",
            "<redacted-email>",
        ),
        (
            "US Social Security number",
            r"\b\d{3}[- ]\d{2}[- ]\d{4}\b",
            "<redacted-ssn>",
        ),
        (
            "phone number",
            r"(?x)(?:\+?1[-.\s]?)?(?:\(\d{3}\)|\d{3})[-.\s]\d{3}[-.\s]\d{4}\b",
            "<redacted-phone>",
        ),
        (
            "local filesystem path",
            r"(?i)(?:/Users/|/home/)[^\s,;]+|\b[A-Z]:\\Users\\[^\s,;]+",
            "<redacted-local-path>",
        ),
        (
            "patient identifier",
            r"(?i)\bpatient\s+(?:id\s*[:#=-]?\s*)?[A-Z0-9-]{4,}\b",
            "<redacted-patient-id>",
        ),
        (
            "medical record identifier",
            r"(?i)\b(?:mrn|medical\s+record(?:\s+number)?)\s*[:#=-]?\s*[A-Z0-9-]{4,}\b",
            "<redacted-medical-record-id>",
        ),
        (
            "date of birth",
            r"(?i)\b(?:dob|date\s+of\s+birth)\s*[:#=-]?\s*(?:\d{1,2}[-/]\d{1,2}[-/]\d{2,4}|[A-Z]+\s+\d{1,2},?\s+\d{4})\b",
            "<redacted-date-of-birth>",
        ),
        (
            "credential or secret",
            r"(?i)\b(?:password|passwd|api[_ -]?key|access[_ -]?token|secret)\s*[:=]\s*[^\s,;]+",
            "<redacted-secret>",
        ),
    ] {
        let regex = Regex::new(pattern).expect("privacy regex is valid");
        let count = regex.find_iter(&redacted).count();
        if count > 0 {
            indicators.push(label.to_string());
            redaction_count += count;
            redacted = regex.replace_all(&redacted, replacement).into_owned();
        }
    }

    let normalized = normalize(query);
    let medical_context = contains_any(
        &normalized,
        &[
            "diagnosis",
            "diagnosed",
            "symptom",
            "symptoms",
            "medication",
            "medicine",
            "drug",
            "insulin",
            "warfarin",
            "antibiotic",
            "antidepressant",
            "dose",
            "dosage",
            "dosing",
            "units",
            "regimen",
            "what amount",
            "appropriate for me",
            "appropriate for my",
            "how much do i take",
            "how much should i take",
            "prescription",
            "patient",
            "medical record",
            "test result",
            "lab result",
            "blood pressure",
            "pregnant",
            "seizure",
            "heart attack",
            "bleeding heavily",
            "severe bleeding",
            "feel faint",
            "unconscious",
            "unresponsive",
            "poisoned",
            "poisoning",
            "suicidal",
            "kill myself",
            "turning blue",
            "gasping",
        ],
    );
    let personal_context = contains_any(
        &normalized,
        &[
            "i have",
            "i am",
            "i think",
            "do i take",
            "should i take",
            "i take",
            "i give",
            "my patient",
            "my child",
            "my baby",
            "my brother",
            "my sister",
            "my mother",
            "my father",
            "my spouse",
            "my medical",
            "their medical",
            "for me",
            "for my child",
            "for my baby",
        ],
    );
    let explicit_restricted_context = contains_any(
        &normalized,
        &[
            "classified",
            "confidential",
            "sealed record",
            "student record",
            "ferpa",
            "private investigation",
        ],
    );
    let academic_material_context = contains_any(
        &normalized,
        &[
            "my professor notes",
            "professor notes",
            "my course notes",
            "class notes",
            "lecture notes",
            "professor presentation",
            "course presentation",
        ],
    );
    let pattern_of_life_context = contains_any(
        &normalized,
        &[
            "residential street",
            "residential address",
            "daily routine",
            "daily schedule",
            "usually sleeps",
            "where they sleep",
            "where he sleeps",
            "where she sleeps",
            "commute from work to home",
            "leave work each day",
            "leaves work each day",
            "parks overnight",
        ],
    ) || (contains_any(
        &normalized,
        &["gps coordinate", "gps coordinates", "precise coordinates"],
    ) && contains_any(
        &normalized,
        &["where", "sleeps", "lives", "home", "residence"],
    )) || (contains_phrase(&normalized, "school")
        && contains_phrase(&normalized, "child")
        && contains_any(&normalized, &["attends", "goes to"]))
        || (contains_phrase(&normalized, "commute")
            && contains_any(&normalized, &["home", "work"]))
        || (contains_any(&normalized, &["leave work", "arrive at work"])
            && contains_any(&normalized, &["each day", "daily", "usually"]))
        || (contains_any(&normalized, &["parks", "parked", "vehicle", "car"])
            && contains_any(&normalized, &["overnight", "each night", "usually"]));

    if medical_context && personal_context {
        indicators.push("personal health context".into());
    }
    if explicit_restricted_context {
        indicators.push("confidential or restricted context".into());
    }
    if academic_material_context {
        indicators.push("local academic material context".into());
    }
    if pattern_of_life_context {
        indicators.push("precise location or pattern-of-life context".into());
    }
    indicators.sort();
    indicators.dedup();

    let highly_sensitive = indicators.iter().any(|indicator| {
        matches!(
            indicator.as_str(),
            "US Social Security number"
                | "credential or secret"
                | "medical record identifier"
                | "patient identifier"
                | "date of birth"
                | "confidential or restricted context"
                | "precise location or pattern-of-life context"
        )
    });
    let level = if highly_sensitive {
        SensitivityLevel::HighlySensitive
    } else if indicators.is_empty() {
        SensitivityLevel::None
    } else {
        SensitivityLevel::Sensitive
    };
    let requires_network_confirmation = level != SensitivityLevel::None;
    let contextual_risk = (medical_context && personal_context)
        || explicit_restricted_context
        || academic_material_context
        || pattern_of_life_context;
    let redacted_query_safe_to_send = level == SensitivityLevel::Sensitive
        && redaction_count > 0
        && !contextual_risk
        && !redacted.contains("<redacted-secret>");
    let guidance = match level {
        SensitivityLevel::None => {
            "No common sensitive-data pattern was detected. This is not a guarantee; review the query before live research.".into()
        }
        SensitivityLevel::Sensitive => {
            "Use offline mode, remove identifying details, or explicitly approve the live request after reviewing the destination disclosure.".into()
        }
        SensitivityLevel::HighlySensitive => {
            "Do not send this query to public connectors. Remove restricted identifiers or keep the work offline.".into()
        }
    };

    PrivacyAssessment {
        level,
        indicators,
        requires_network_confirmation,
        redacted_query: redacted,
        redaction_count,
        redacted_query_safe_to_send,
        guidance,
    }
}

fn normalize(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_any(normalized: &str, phrases: &[&str]) -> bool {
    let padded = format!(" {normalized} ");
    phrases
        .iter()
        .any(|phrase| padded.contains(&format!(" {} ", normalize(phrase))))
}

fn contains_phrase(normalized: &str, phrase: &str) -> bool {
    contains_any(normalized, &[phrase])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_research_is_not_flagged() {
        let result = assess("Who is the current president of Kenya?");
        assert_eq!(result.level, SensitivityLevel::None);
        assert!(!result.requires_network_confirmation);
    }

    #[test]
    fn values_are_redacted_and_never_returned_as_indicators() {
        let query = "Research records for person@example.com and 814-555-0199";
        let result = assess(query);
        assert_eq!(result.level, SensitivityLevel::Sensitive);
        assert!(!result.redacted_query.contains("person@example.com"));
        assert!(!result.redacted_query.contains("814-555-0199"));
        assert_eq!(result.redaction_count, 2);
        assert!(result.redacted_query_safe_to_send);
        assert!(result.indicators.iter().all(|value| !query.contains(value)));
    }

    #[test]
    fn personal_health_context_requires_confirmation_without_fake_redaction() {
        let result = assess("My brother has a new rash and medication symptoms");
        assert_eq!(result.level, SensitivityLevel::Sensitive);
        assert!(result.requires_network_confirmation);
        assert!(!result.redacted_query_safe_to_send);
    }

    #[test]
    fn secrets_and_medical_ids_are_highly_sensitive() {
        for query in [
            "api_key=abcd1234 research this",
            "MRN: AB-12345 symptoms",
            "DOB 01/02/2003 lab result",
            "SSN 123-45-6789",
        ] {
            assert_eq!(assess(query).level, SensitivityLevel::HighlySensitive);
        }
    }

    #[test]
    fn patient_dosing_and_pattern_of_life_are_flagged() {
        let dosing = assess("How much insulin do I take for my child?");
        assert_eq!(dosing.level, SensitivityLevel::Sensitive);
        assert!(!dosing.redacted_query_safe_to_send);

        for query in [
            "Find Jane Example's residential street and daily routine",
            "Give me the GPS coordinates where Jane Example usually sleeps",
            "Locate the school Jane Example's child attends",
            "Map Jane Example commute from work to home",
            "What time does Jane Example leave work each day?",
            "Find where Jane Example parks overnight",
        ] {
            assert_eq!(assess(query).level, SensitivityLevel::HighlySensitive);
        }
    }

    #[test]
    fn treatment_and_emergency_contexts_require_local_review() {
        for query in [
            "How many units of insulin are appropriate for my child?",
            "What amount of warfarin is appropriate for me?",
            "Please determine an insulin regimen for my baby",
            "My child is having a seizure right now",
            "I think I'm having a heart attack",
            "I am bleeding heavily and feel faint",
        ] {
            assert_ne!(assess(query).level, SensitivityLevel::None, "{query}");
        }
    }

    #[test]
    fn local_academic_paths_and_patient_ids_require_review() {
        let notes = assess("Search my professor notes in /Users/alex/Documents/PSYCH-401");
        assert_eq!(notes.level, SensitivityLevel::Sensitive);
        assert!(!notes.redacted_query.contains("/Users/alex"));
        assert!(!notes.redacted_query_safe_to_send);

        let patient = assess("Compare discharge notes for patient A12345 with these symptoms");
        assert_eq!(patient.level, SensitivityLevel::HighlySensitive);
        assert!(!patient.redacted_query.contains("A12345"));
        assert!(!patient.redacted_query_safe_to_send);
    }
}
