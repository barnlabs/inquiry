use crate::http::json_limited;
use crate::policy::review_query;
use crate::privacy::assess as assess_privacy;
use crate::sources::default_client;
use anyhow::{Context, Result, bail};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const NOMINATIM_URL: &str = "https://nominatim.openstreetmap.org/search";
const NOMINATIM_INTERVAL: Duration = Duration::from_millis(1_050);
static NOMINATIM_SCHEDULE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
struct SearchBounds<'a> {
    latitude: f64,
    longitude: f64,
    country_code: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceResolution {
    pub query: String,
    pub target_query: String,
    pub anchor_query: Option<String>,
    pub anchor: Option<PlaceCandidate>,
    pub anchor_candidates: Vec<PlaceCandidate>,
    pub candidates: Vec<PlaceCandidate>,
    pub warnings: Vec<String>,
    pub attribution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceCandidate {
    pub label: String,
    pub latitude: f64,
    pub longitude: f64,
    pub osm_type: String,
    pub osm_id: u64,
    pub category: Option<String>,
    pub kind: Option<String>,
    pub importance: Option<f64>,
    pub address: BTreeMap<String, String>,
    pub distance_to_anchor_meters: Option<f64>,
    pub map_url: String,
    pub verification_signals: Vec<String>,
}

pub async fn resolve(query: &str, limit: usize, network_allowed: bool) -> Result<PlaceResolution> {
    let query = query.trim();
    if query.len() < 3 {
        bail!("place query must contain at least three characters");
    }
    if query.chars().count() > 500 {
        bail!("place query must not exceed 500 characters");
    }
    if !network_allowed {
        bail!("place resolution is unavailable in offline mode because it requires Nominatim");
    }
    let policy = review_query(query);
    if !policy.allowed {
        bail!(
            policy
                .reason
                .unwrap_or_else(|| "place query declined by policy".into())
        );
    }
    let privacy = assess_privacy(query);
    if privacy.requires_network_confirmation {
        bail!(
            "sensitive or precise-location context detected; place resolution will not send it to public Nominatim. Remove private details and retry with a public place or institution"
        );
    }
    let client = default_client()?;
    let (target, anchor_query) = split_near(query);
    let mut anchor = None;
    let mut anchor_candidates = Vec::new();
    let target_query = if let Some(anchor_text) = anchor_query {
        anchor_candidates = search_once(&client, anchor_text, 5, None).await?;
        anchor = anchor_candidates.first().cloned();
        if anchor_is_ambiguous(&anchor_candidates) {
            let mut warnings = base_warnings(policy.warnings);
            warnings.insert(0, "The nearby landmark or locality is ambiguous. No target search was performed; rerun with city, state/region, and country.".into());
            return Ok(PlaceResolution {
                query: query.to_string(),
                target_query: target.to_string(),
                anchor_query: Some(anchor_text.to_string()),
                anchor,
                anchor_candidates,
                candidates: Vec::new(),
                warnings,
                attribution: attribution(),
            });
        }
        anchor
            .as_ref()
            .map(place_context)
            .filter(|context| !context.is_empty())
            .map(|context| format!("{target}, {context}"))
            .unwrap_or_else(|| target.to_string())
    } else {
        target.to_string()
    };
    let bounds = anchor.as_ref().map(|candidate| SearchBounds {
        latitude: candidate.latitude,
        longitude: candidate.longitude,
        country_code: candidate.address.get("country_code").map(String::as_str),
    });
    let mut candidates = search_once(&client, &target_query, limit.clamp(1, 10), bounds).await?;
    candidates.retain(|candidate| candidate_matches_target(target, candidate));
    if let Some(reference) = &anchor {
        for candidate in &mut candidates {
            let distance = haversine_meters(
                candidate.latitude,
                candidate.longitude,
                reference.latitude,
                reference.longitude,
            );
            candidate.distance_to_anchor_meters = Some(distance);
            candidate.verification_signals.push(format!(
                "approximately {} from anchor",
                human_distance(distance)
            ));
        }
        candidates.sort_by(|a, b| {
            a.distance_to_anchor_meters
                .unwrap_or(f64::INFINITY)
                .total_cmp(&b.distance_to_anchor_meters.unwrap_or(f64::INFINITY))
        });
    }
    Ok(PlaceResolution {
        query: query.to_string(),
        target_query,
        anchor_query: anchor_query.map(str::to_string),
        anchor,
        anchor_candidates,
        candidates,
        warnings: base_warnings(policy.warnings),
        attribution: attribution(),
    })
}

fn base_warnings(policy_warnings: Vec<String>) -> Vec<String> {
    policy_warnings.into_iter().chain([
        "Candidates are not asserted matches. Verify the address, map position, OSM object, and nearby landmarks before using a result.".into(),
        "OpenStreetMap records can be incomplete or stale; sensitive or consequential location decisions need an additional authoritative source.".into(),
        "Public Nominatim is user-triggered discovery only and is not used for bulk geocoding or autocomplete.".into(),
        "Queries using 'near' constrain target candidates to a local search window around the resolved anchor; an empty result is preferred to a distant global match.".into(),
        "Reported distances are great-circle distances between OSM representative points, not travel distance or emergency routing.".into(),
    ]).collect()
}

fn attribution() -> String {
    "Place data © OpenStreetMap contributors, ODbL 1.0; geocoding by the public Nominatim service."
        .into()
}

fn candidate_matches_target(target: &str, candidate: &PlaceCandidate) -> bool {
    let normalized = target.to_lowercase();
    let accepted_kinds: &[&str] = if normalized.split_whitespace().any(|word| word == "hospital") {
        &["hospital", "clinic"]
    } else if normalized.split_whitespace().any(|word| word == "pharmacy") {
        &["pharmacy"]
    } else if normalized
        .split_whitespace()
        .any(|word| word == "university")
    {
        &["university", "college"]
    } else {
        return true;
    };
    candidate
        .kind
        .as_deref()
        .is_some_and(|kind| accepted_kinds.contains(&kind))
}

async fn search_once(
    client: &reqwest::Client,
    query: &str,
    limit: usize,
    bounds: Option<SearchBounds<'_>>,
) -> Result<Vec<PlaceCandidate>> {
    wait_for_nominatim_slot().await?;
    let mut parameters = vec![
        ("q".to_string(), query.to_string()),
        ("format".into(), "jsonv2".into()),
        ("addressdetails".into(), "1".into()),
        ("namedetails".into(), "1".into()),
        ("limit".into(), limit.to_string()),
    ];
    if let Some(bounds) = bounds {
        let latitude_delta = 0.75;
        let longitude_delta = 0.75;
        parameters.push((
            "viewbox".into(),
            format!(
                "{},{},{},{}",
                bounds.longitude - longitude_delta,
                bounds.latitude + latitude_delta,
                bounds.longitude + longitude_delta,
                bounds.latitude - latitude_delta
            ),
        ));
        parameters.push(("bounded".into(), "1".into()));
        if let Some(country_code) = bounds.country_code {
            parameters.push(("countrycodes".into(), country_code.to_lowercase()));
        }
    }
    let response = client
        .get(nominatim_endpoint()?)
        .query(&parameters)
        .header("Accept-Language", "en")
        .send()
        .await
        .context("Nominatim request failed")?
        .error_for_status()
        .context("Nominatim rejected the place request")?;
    let response: Vec<Value> = json_limited(response, 2_000_000, "Nominatim")
        .await
        .context("Nominatim returned unreadable JSON")?;
    response
        .into_iter()
        .map(candidate_from_value)
        .collect::<Result<Vec<_>>>()
}

fn place_context(candidate: &PlaceCandidate) -> String {
    ["city", "town", "village", "state", "country"]
        .iter()
        .filter_map(|key| candidate.address.get(*key))
        .fold(Vec::<&str>::new(), |mut values, value| {
            if !values.contains(&value.as_str()) {
                values.push(value);
            }
            values
        })
        .join(", ")
}

async fn wait_for_nominatim_slot() -> Result<()> {
    let now = Instant::now();
    let scheduled = {
        let mut last = NOMINATIM_SCHEDULE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let scheduled = last
            .map(|previous| previous + NOMINATIM_INTERVAL)
            .filter(|candidate| *candidate > now)
            .unwrap_or(now);
        *last = Some(scheduled);
        scheduled
    };
    if scheduled > now {
        tokio::time::sleep(scheduled - now).await;
    }
    tokio::task::spawn_blocking(|| -> Result<()> {
        enforce_cross_process_nominatim_slot(&nominatim_rate_limit_path())
    })
    .await
    .context("Nominatim rate-limit task failed")??;
    Ok(())
}

fn nominatim_rate_limit_path() -> PathBuf {
    let effective_user = unsafe { libc::geteuid() };
    std::env::temp_dir().join(format!(
        "barnlabs-inquiry-{effective_user}-nominatim-rate-limit"
    ))
}

fn enforce_cross_process_nominatim_slot(path: &Path) -> Result<()> {
    let effective_user = unsafe { libc::geteuid() };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .with_context(|| format!("could not open rate-limit state at {}", path.display()))?;
    file.lock_exclusive()
        .context("could not acquire cross-process Nominatim rate-limit lock")?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != effective_user
        || metadata.nlink() != 1
        || metadata.mode() & 0o077 != 0
        || metadata.len() > 64
    {
        bail!(
            "Nominatim rate-limit state must be one owner-only regular file no larger than 64 bytes"
        );
    }
    let mut previous = String::new();
    (&mut file).take(64).read_to_string(&mut previous)?;
    let previous_millis = if previous.trim().is_empty() {
        0
    } else {
        previous
            .trim()
            .parse::<u128>()
            .context("Nominatim rate-limit state was not a valid timestamp")?
    };
    let mut now_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let maximum_future = now_millis.saturating_add(NOMINATIM_INTERVAL.as_millis() * 5);
    if previous_millis > maximum_future {
        bail!(
            "Nominatim rate-limit state was implausibly far in the future; Inquiry refused an unbounded wait"
        );
    }
    let required = previous_millis.saturating_add(NOMINATIM_INTERVAL.as_millis());
    if required > now_millis {
        std::thread::sleep(Duration::from_millis((required - now_millis) as u64));
        now_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
    }
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    write!(file, "{now_millis}")?;
    file.sync_data()?;
    file.unlock()?;
    Ok(())
}

fn nominatim_endpoint() -> Result<String> {
    let value = std::env::var("INQUIRY_NOMINATIM_URL").unwrap_or_else(|_| NOMINATIM_URL.into());
    let parsed = url::Url::parse(&value).context("INQUIRY_NOMINATIM_URL is not a valid URL")?;
    let local_http = parsed.scheme() == "http"
        && parsed
            .host_str()
            .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "::1"));
    if parsed.scheme() != "https" && !local_http {
        bail!("Nominatim endpoint must use HTTPS (HTTP is allowed only for loopback development)");
    }
    Ok(value)
}

fn candidate_from_value(value: Value) -> Result<PlaceCandidate> {
    let label = value
        .get("display_name")
        .and_then(Value::as_str)
        .unwrap_or("Unnamed place")
        .to_string();
    let latitude = parse_number(&value, "lat")?;
    let longitude = parse_number(&value, "lon")?;
    let osm_type = value
        .get("osm_type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let osm_id = value
        .get("osm_id")
        .and_then(Value::as_u64)
        .context("place candidate did not include an OSM identifier")?;
    let address: BTreeMap<String, String> = value
        .get("address")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| value.as_str().map(|text| (key.clone(), text.into())))
                .collect()
        })
        .unwrap_or_default();
    let mut verification_signals = vec![
        format!("OSM {osm_type} {osm_id}"),
        format!("coordinates {latitude:.6}, {longitude:.6}"),
    ];
    for key in [
        "house_number",
        "road",
        "city",
        "town",
        "state",
        "postcode",
        "country",
    ] {
        if let Some(component) = address.get(key) {
            verification_signals.push(format!("{key}: {component}"));
        }
    }
    let type_code = match osm_type.as_str() {
        "node" => "N",
        "way" => "W",
        "relation" => "R",
        _ => "",
    };
    let map_url = if type_code.is_empty() {
        format!("https://www.openstreetmap.org/?mlat={latitude}&mlon={longitude}")
    } else {
        format!("https://www.openstreetmap.org/{osm_type}/{osm_id}")
    };
    Ok(PlaceCandidate {
        label,
        latitude,
        longitude,
        osm_type,
        osm_id,
        category: value
            .get("category")
            .and_then(Value::as_str)
            .map(str::to_string),
        kind: value
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_string),
        importance: value.get("importance").and_then(Value::as_f64),
        address,
        distance_to_anchor_meters: None,
        map_url,
        verification_signals,
    })
}

fn parse_number(value: &Value, key: &str) -> Result<f64> {
    value
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("place candidate did not include {key}"))?
        .parse()
        .with_context(|| format!("place candidate had invalid {key}"))
}

fn split_near(query: &str) -> (&str, Option<&str>) {
    let lower = query.to_lowercase();
    if let Some(index) = lower.find(" near ") {
        let target = query[..index].trim();
        let anchor = query[index + 6..].trim();
        if !target.is_empty() && !anchor.is_empty() {
            return (target, Some(anchor));
        }
    }
    (query, None)
}

fn anchor_is_ambiguous(candidates: &[PlaceCandidate]) -> bool {
    candidates.len() > 1
}

fn haversine_meters(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let radius = 6_371_008.8_f64;
    let (lat1, lat2) = (lat1.to_radians(), lat2.to_radians());
    let delta_lat = lat2 - lat1;
    let delta_lon = (lon2 - lon1).to_radians();
    let a =
        (delta_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
    2.0 * radius * a.sqrt().asin()
}

fn human_distance(meters: f64) -> String {
    if meters < 1_000.0 {
        format!("{meters:.0} m")
    } else {
        format!("{:.2} km", meters / 1_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn parses_near_queries_without_guessing_other_prepositions() {
        assert_eq!(
            split_near("Walmart near Home Depot, Erie, Pennsylvania"),
            ("Walmart", Some("Home Depot, Erie, Pennsylvania"))
        );
        assert_eq!(split_near("Walmart in Erie"), ("Walmart in Erie", None));
    }

    #[test]
    fn distance_matches_known_short_baseline() {
        let meters = haversine_meters(42.1292, -80.0851, 42.1269, -80.0815);
        assert!((meters - 390.0).abs() < 50.0);
    }

    #[test]
    fn comma_does_not_suppress_anchor_ambiguity() {
        let candidate = |label: &str| PlaceCandidate {
            label: label.into(),
            latitude: 0.0,
            longitude: 0.0,
            osm_type: "relation".into(),
            osm_id: 1,
            category: None,
            kind: None,
            importance: None,
            address: BTreeMap::new(),
            distance_to_anchor_meters: None,
            map_url: "https://www.openstreetmap.org/relation/1".into(),
            verification_signals: vec![],
        };
        assert!(anchor_is_ambiguous(&[
            candidate("Springfield, Illinois, United States"),
            candidate("Springfield, Missouri, United States"),
        ]));
    }

    #[test]
    fn rate_limit_state_rejects_symlinks_without_touching_the_target() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target");
        let link = directory.path().join("rate-limit");
        std::fs::write(&target, b"sentinel").unwrap();
        symlink(&target, &link).unwrap();

        assert!(enforce_cross_process_nominatim_slot(&link).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"sentinel");
    }

    #[test]
    fn rate_limit_state_rejects_an_unbounded_future_wait() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rate-limit");
        std::fs::write(&path, u128::MAX.to_string()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

        let error = enforce_cross_process_nominatim_slot(&path).unwrap_err();
        assert!(error.to_string().contains("future"));
    }
}
