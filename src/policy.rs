use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub warnings: Vec<String>,
    pub reason: Option<String>,
}

pub fn review_query(query: &str) -> PolicyDecision {
    let normalized = normalize(query);
    let disallowed = [
        "social security number",
        "steal credentials",
        "find their password",
        "home address of",
        "dox",
        "doxx",
        "stalk",
        "track them live",
        "live location of",
        "current location of",
        "real-time location of",
        "private medical record",
    ];
    let residence_targeting = contains_any(&normalized, &["locate", "find", "address"])
        && contains_any(
            &normalized,
            &[
                "residence",
                "home address",
                "residential address",
                "residential street",
                "lives at",
            ],
        );
    let pattern_of_life_targeting = contains_any(
        &normalized,
        &["find", "locate", "track", "map", "identify", "give me"],
    ) && contains_any(
        &normalized,
        &[
            "daily routine",
            "daily schedule",
            "usual routine",
            "usually sleeps",
            "where they sleep",
            "where he sleeps",
            "where she sleeps",
            "work schedule",
            "residential street",
            "residential address",
            "commute from work to home",
            "leave work each day",
            "leaves work each day",
            "parks overnight",
            "parked overnight",
            "school their child attends",
            "school his child attends",
            "school her child attends",
        ],
    );
    let sensitive_association_targeting =
        (contains_any(&normalized, &["locate", "find", "identify"])
            && contains_phrase(&normalized, "school")
            && contains_phrase(&normalized, "child")
            && contains_any(&normalized, &["attends", "goes to"]))
            || (contains_any(&normalized, &["map", "track", "find"])
                && contains_phrase(&normalized, "commute")
                && contains_any(&normalized, &["home", "work"]))
            || (contains_any(&normalized, &["what time", "when"])
                && contains_any(&normalized, &["leave work", "arrive at work"])
                && contains_any(&normalized, &["each day", "daily", "usually"]))
            || (contains_any(&normalized, &["find", "locate", "where"])
                && contains_any(&normalized, &["parks", "parked", "vehicle", "car"])
                && contains_any(&normalized, &["overnight", "each night", "usually"]));
    let precise_location_targeting = contains_any(
        &normalized,
        &["gps coordinate", "gps coordinates", "precise coordinates"],
    ) && contains_any(
        &normalized,
        &[
            "where",
            "sleeps",
            "lives",
            "home",
            "residence",
            "current location",
            "usual location",
        ],
    );
    if (contains_phrase(&normalized, "where does") && contains_phrase(&normalized, "live"))
        || residence_targeting
        || pattern_of_life_targeting
        || sensitive_association_targeting
        || precise_location_targeting
    {
        return PolicyDecision {
            allowed: false,
            warnings: Vec::new(),
            reason: Some(
                "Inquiry does not resolve a person's residence, precise location, or pattern of life."
                    .into(),
            ),
        };
    }
    if let Some(term) = disallowed
        .iter()
        .find(|term| contains_phrase(&normalized, term))
    {
        return PolicyDecision {
            allowed: false,
            warnings: Vec::new(),
            reason: Some(format!(
                "The request includes sensitive targeting ('{term}'). Inquiry supports lawful public-interest research, not credential theft, doxxing, stalking, or access to private records."
            )),
        };
    }
    let intrusion = contains_any(&normalized, &["break into", "hack into", "bypass access"])
        && contains_any(
            &normalized,
            &[
                "account", "computer", "system", "database", "network", "server",
            ],
        );
    if intrusion {
        return PolicyDecision {
            allowed: false,
            warnings: Vec::new(),
            reason: Some(
                "Inquiry does not support unauthorized access or credential compromise.".into(),
            ),
        };
    }
    let breathing_emergency = contains_any(
        &normalized,
        &[
            "i can t breathe",
            "i cannot breathe",
            "i can not breathe",
            "unable to breathe",
            "not breathing",
            "stopped breathing",
            "choking right now",
        ],
    );
    let emergency_personal_context = contains_any(
        &normalized,
        &[
            "i am",
            "i m",
            "i think",
            "i have",
            "my child",
            "my baby",
            "my brother",
            "my sister",
            "my mother",
            "my father",
            "someone is",
            "they are",
            "he is",
            "she is",
        ],
    );
    let emergency_condition = contains_any(
        &normalized,
        &[
            "having a seizure",
            "seizure right now",
            "heart attack",
            "bleeding heavily",
            "severe bleeding",
            "feel faint",
            "fainted",
            "unconscious",
            "unresponsive",
            "poisoned",
            "poisoning",
            "swallowed poison",
            "suicidal",
            "kill myself",
            "end my life",
            "turned blue",
            "turning blue",
            "gasping for air",
            "gasping",
        ],
    );
    let possible_emergency = breathing_emergency
        || (emergency_personal_context && emergency_condition)
        || (contains_phrase(&normalized, "chest pain")
            && contains_any(
                &normalized,
                &[
                    "shortness of breath",
                    "difficulty breathing",
                    "can t breathe",
                ],
            ))
        || contains_any(
            &normalized,
            &[
                "signs of a stroke",
                "overdose right now",
                "severe allergic reaction",
                "anaphylaxis",
            ],
        );
    if possible_emergency {
        return PolicyDecision {
            allowed: false,
            warnings: Vec::new(),
            reason: Some("This query may describe a medical emergency. Inquiry cannot triage it. Call emergency services now (911 in the United States) or your local emergency number; do not wait for a research report.".into()),
        };
    }
    let personal_dosing_request = (contains_phrase(&normalized, "how much")
        && contains_any(
            &normalized,
            &["take", "give", "inject", "administer", "use"],
        ))
        || contains_any(
            &normalized,
            &[
                "what dose",
                "dose should",
                "dosing for me",
                "dosing for my",
                "how many units",
                "what amount",
                "determine a regimen",
                "determine an regimen",
                "appropriate for me",
                "appropriate for my",
            ],
        )
        || (contains_phrase(&normalized, "determine") && contains_phrase(&normalized, "regimen"));
    let medication_or_treatment_context = contains_any(
        &normalized,
        &[
            "medication",
            "medicine",
            "drug",
            "insulin",
            "warfarin",
            "antibiotic",
            "antidepressant",
            "painkiller",
            "dose",
            "dosage",
            "dosing",
            "units",
            "regimen",
            "frequency",
            "inject",
            "injection",
            "tablet",
            "pill",
            "prescription",
        ],
    );
    let medical_action = personal_dosing_request
        || contains_any(
            &normalized,
            &[
                "what dose",
                "how much should",
                "should i take",
                "should i give",
                "can i take",
                "diagnose me",
                "diagnose my",
                "prescribe",
            ],
        );
    let personal_medical_context = contains_any(
        &normalized,
        &[
            "my child",
            "my baby",
            "for my child",
            "for my baby",
            "my symptoms",
            "for me",
            "do i",
            "should i",
            "can i",
            "i take",
            "i give",
        ],
    );
    if medical_action && medication_or_treatment_context && personal_medical_context {
        return PolicyDecision {
            allowed: false,
            warnings: Vec::new(),
            reason: Some("Inquiry cannot provide patient-specific dosing, diagnosis, prescribing, or treatment decisions. Use a qualified clinician or emergency service as appropriate.".into()),
        };
    }

    let mut warnings = Vec::new();
    if [
        "disease",
        "symptom",
        "symptoms",
        "treatment",
        "diagnose",
        "diagnosed",
        "diagnosis",
        "medicine",
        "medication",
        "drug",
        "dose",
        "insulin",
        "interaction",
        "transmission",
    ]
    .iter()
    .any(|term| contains_phrase(&normalized, term))
    {
        warnings.push("Health information is evidence support, not diagnosis or treatment advice. Verify clinical decisions with current official guidance and a qualified professional.".to_string());
    }
    if [
        "stock",
        "investment",
        "financial",
        "revenue",
        "gdp",
        "price target",
    ]
    .iter()
    .any(|term| contains_phrase(&normalized, term))
    {
        warnings.push("Financial data may be delayed, revised, or definition-dependent. This report is research, not a transaction recommendation.".to_string());
    }
    if ["person", "people", "employee", "citizen", "officer"]
        .iter()
        .any(|term| contains_phrase(&normalized, term))
    {
        warnings.push("Use person-related information only for a lawful, proportionate purpose; minimize collection and independently verify identity matches.".to_string());
    }

    PolicyDecision {
        allowed: true,
        warnings,
        reason: None,
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

fn contains_phrase(normalized: &str, phrase: &str) -> bool {
    let phrase = normalize(phrase);
    format!(" {normalized} ").contains(&format!(" {phrase} "))
}

fn contains_any(normalized: &str, phrases: &[&str]) -> bool {
    phrases
        .iter()
        .any(|phrase| contains_phrase(normalized, phrase))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_doxxing() {
        let decision = review_query("Find the home address of a private person");
        assert!(!decision.allowed);
    }

    #[test]
    fn warns_on_health() {
        let decision = review_query("How is dengue disease transmitted?");
        assert!(decision.allowed);
        assert!(
            decision
                .warnings
                .iter()
                .any(|w| w.contains("not diagnosis"))
        );
    }

    #[test]
    fn blocks_residence_and_live_location_targeting() {
        assert!(!review_query("Where does Jane Example live?").allowed);
        assert!(!review_query("Find the current location of a private person").allowed);
        assert!(!review_query("Locate Jane Example's residence").allowed);
        assert!(!review_query("Find Jane Example's residential street and daily routine").allowed);
        assert!(
            !review_query("Give me the GPS coordinates where Jane Example usually sleeps").allowed
        );
        assert!(review_query("Explain how GPS coordinates work").allowed);
        assert!(review_query("Research daily routines in psychology").allowed);
        for query in [
            "Locate the school Jane Example's child attends",
            "Map Jane Example commute from work to home",
            "What time does Jane Example leave work each day?",
            "Find where Jane Example parks overnight",
        ] {
            assert!(
                !review_query(query).allowed,
                "query should be blocked: {query}"
            );
        }
    }

    #[test]
    fn allows_words_that_only_contain_blocked_fragments() {
        assert!(review_query("Explain redox chemistry").allowed);
        assert!(review_query("How do I break into cybersecurity as a career?").allowed);
    }

    #[test]
    fn blocks_patient_specific_dosing() {
        assert!(!review_query("What dose of insulin should I give my child?").allowed);
        assert!(!review_query("How much insulin do I take for my child?").allowed);
        assert!(review_query("Find official pediatric insulin dosing guidelines").allowed);
        assert!(!review_query("Can I take warfarin with ibuprofen?").allowed);
        assert!(!review_query("How many units of insulin are appropriate for my child?").allowed);
        assert!(!review_query("What amount of warfarin is appropriate for me?").allowed);
        assert!(!review_query("Please determine an insulin regimen for my baby").allowed);
    }

    #[test]
    fn redirects_possible_emergencies() {
        let decision =
            review_query("I have crushing chest pain and shortness of breath, what should I do?");
        assert!(!decision.allowed);
        assert!(decision.reason.unwrap().contains("emergency services"));
        assert!(!review_query("I can't breathe, what should I do?").allowed);
        assert!(!review_query("My child is not breathing").allowed);
        for query in [
            "My child is having a seizure right now",
            "I think I'm having a heart attack",
            "I am bleeding heavily and feel faint",
            "My baby is unconscious",
            "I think my brother was poisoned",
            "I am suicidal and might kill myself",
            "My child is turning blue and gasping",
        ] {
            assert!(
                !review_query(query).allowed,
                "query should redirect: {query}"
            );
        }
    }
}
