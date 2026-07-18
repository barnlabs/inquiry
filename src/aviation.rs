use crate::http::json_limited;
use crate::sources::default_client;
use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use clap::ValueEnum;
use reqwest::Client;
use reqwest::header::RETRY_AFTER;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::str::FromStr;
use std::time::Instant;
use url::Url;
use uuid::Uuid;
use zip::ZipArchive;

pub const FAA_AIRPORT_EVENTS_URL: &str = "https://nasstatus.faa.gov/api/airport-events";
pub const FAA_NAS_STATUS_PAGE: &str = "https://nasstatus.faa.gov/";
pub const FAA_REGISTRY_DOWNLOAD_PAGE: &str = "https://www.faa.gov/licenses_certificates/aircraft_certification/aircraft_registry/releasable_aircraft_download";
pub const FAA_REGISTRY_DOCUMENTATION: &str = "https://registry.faa.gov/database/ardata.pdf";

#[derive(Debug, Clone, Serialize)]
pub struct AirportStatus {
    pub schema_version: &'static str,
    pub airport_id: String,
    pub airport_name: Option<String>,
    pub retrieved_at: DateTime<Utc>,
    pub source_updated_at: DateTime<Utc>,
    pub source_age_seconds: i64,
    pub freshness_statement: String,
    pub active_events: Vec<AirportEvent>,
    pub status_statement: String,
    pub source_url: &'static str,
    pub source_page: &'static str,
    pub connector: &'static str,
    pub network_used: bool,
    pub latency_ms: u128,
    pub warning: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct AirportEvent {
    pub event_type: String,
    pub reason: Option<String>,
    pub average_delay_minutes: Option<f64>,
    pub maximum_delay_minutes: Option<f64>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub source_timestamp: Option<DateTime<Utc>>,
    pub advisory_url: Option<String>,
}

pub async fn airport_status(airport: &str, network_allowed: bool) -> Result<AirportStatus> {
    if !network_allowed {
        bail!("airport status is unavailable offline; Inquiry did not contact the FAA");
    }
    let client = default_client()?;
    airport_status_with_client(&client, FAA_AIRPORT_EVENTS_URL, airport).await
}

async fn airport_status_with_client(
    client: &Client,
    endpoint: &str,
    airport: &str,
) -> Result<AirportStatus> {
    let airport_id = normalize_airport_id(airport)?;
    let endpoint_url = Url::parse(endpoint)?;
    if endpoint_url.scheme() != "https"
        || endpoint_url.host_str() != Some("nasstatus.faa.gov")
        || endpoint_url.path() != "/api/airport-events"
    {
        bail!("FAA airport connector destination is not on the exact allowlist");
    }
    let started = Instant::now();
    let response = client
        .get(endpoint_url.clone())
        .send()
        .await
        .context("FAA NAS airport-status request failed")?;
    if response.status().as_u16() == 429 {
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.chars().take(64).collect::<String>());
        bail!(rate_limit_message(retry_after.as_deref()));
    }
    if response.status().as_u16() == 503 {
        bail!(
            "FAA NAS airport-status service is temporarily unavailable; Inquiry did not retry automatically"
        );
    }
    let response = response
        .error_for_status()
        .context("FAA NAS airport-status request was rejected")?;
    if response.url() != &endpoint_url {
        bail!("FAA NAS airport-status response redirected outside the exact endpoint");
    }
    let value: Value = json_limited(response, 1_048_576, "FAA NAS airport status").await?;
    parse_airport_events(
        value,
        &airport_id,
        Utc::now(),
        started.elapsed().as_millis(),
    )
}

fn rate_limit_message(retry_after: Option<&str>) -> String {
    match retry_after {
        Some(value) => format!(
            "FAA NAS airport-status rate limit reached; do not retry before Retry-After {value}"
        ),
        None => {
            "FAA NAS airport-status rate limit reached; Inquiry did not retry automatically".into()
        }
    }
}

fn normalize_airport_id(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|value| value.is_ascii_alphabetic()) {
        bail!("airport must be one three-letter U.S. airport identifier such as ATL or JFK");
    }
    Ok(normalized)
}

fn parse_airport_events(
    value: Value,
    airport_id: &str,
    retrieved_at: DateTime<Utc>,
    latency_ms: u128,
) -> Result<AirportStatus> {
    let rows = value
        .as_array()
        .ok_or_else(|| anyhow!("FAA NAS airport-status response was not an array"))?;
    if rows.len() > 2_000 {
        bail!("FAA NAS airport-status response exceeded the 2,000-airport limit");
    }
    let row = rows.iter().find(|row| {
        row.get("airportId")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(airport_id))
    });
    let row = row.ok_or_else(|| {
        anyhow!(
            "FAA NAS snapshot did not contain an airport record for {airport_id}; Inquiry cannot infer current status"
        )
    })?;
    let airport_name = row
        .get("airportLongName")
        .and_then(Value::as_str)
        .map(|value| truncate(value, 160));
    let mut events = Vec::new();
    for (field, label) in [
        ("groundStop", "Ground stop"),
        ("groundDelay", "Ground delay program"),
        ("departureDelay", "Departure delay"),
        ("arrivalDelay", "Arrival delay"),
        ("airportClosure", "Airport closure"),
        ("deicing", "Deicing event"),
        ("freeForm", "FAA airport advisory"),
    ] {
        if let Some(event) = row.get(field).filter(|value| value.is_object()) {
            let event = parse_airport_event(event, label)?;
            let has_started = event.start_time.is_none_or(|time| time <= retrieved_at);
            let has_not_ended = event.end_time.is_none_or(|time| time >= retrieved_at);
            if has_started && has_not_ended {
                events.push(event);
            }
        }
    }
    let event_updated_at = events
        .iter()
        .filter_map(|event| event.source_timestamp)
        .max();
    let configuration_updated_at = row
        .get("airportConfig")
        .map(|value| timestamp_checked(value, &["sourceTimeStamp", "updatedAt", "createdAt"]))
        .transpose()?
        .flatten();
    let source_updated_at = event_updated_at
        .into_iter()
        .chain(configuration_updated_at)
        .max()
        .ok_or_else(|| {
            anyhow!(
                "FAA NAS airport record for {airport_id} omitted a source timestamp; Inquiry abstained from a current-status interpretation"
            )
        })?;
    if source_updated_at > retrieved_at + Duration::minutes(5) {
        bail!(
            "FAA NAS airport record for {airport_id} supplied a source timestamp more than five minutes in the future; Inquiry abstained from a current-status interpretation"
        );
    }
    let source_age_seconds = retrieved_at
        .signed_duration_since(source_updated_at)
        .num_seconds()
        .max(0);
    if source_age_seconds > 86_400 {
        bail!(
            "FAA NAS airport record for {airport_id} was more than 24 hours old; Inquiry abstained from a current-status interpretation"
        );
    }
    let freshness_statement = if source_age_seconds <= 21_600 {
        format!(
            "The latest accepted FAA source timestamp was {source_age_seconds} seconds before retrieval."
        )
    } else if events.is_empty() {
        format!(
            "The latest accepted FAA source timestamp was {source_age_seconds} seconds before retrieval; the empty event list must not be read as proof of normal operations."
        )
    } else {
        format!(
            "The latest accepted FAA source timestamp was {source_age_seconds} seconds before retrieval; every returned event also had a validity window that included retrieval time or no supplied boundary."
        )
    };
    let status_statement = if events.is_empty() {
        format!(
            "The FAA snapshot contained no active listed delay, closure, deicing, or advisory event for {airport_id}. This does not prove normal operations and is not an individual flight status."
        )
    } else {
        format!(
            "The FAA snapshot listed {} active airport-level event(s) for {airport_id}.",
            events.len()
        )
    };
    Ok(AirportStatus {
        schema_version: "inquiry.airport-status/v1",
        airport_id: airport_id.into(),
        airport_name,
        retrieved_at,
        source_updated_at,
        source_age_seconds,
        freshness_statement,
        active_events: events,
        status_statement,
        source_url: FAA_AIRPORT_EVENTS_URL,
        source_page: FAA_NAS_STATUS_PAGE,
        connector: "FAA National Airspace System status",
        network_used: true,
        latency_ms,
        warning: "Airport-level traffic-management information is not an airline's individual flight status and is not suitable for navigation, dispatch, or safety-of-flight decisions. Verify with the airline and FAA operational channels.",
    })
}

fn parse_airport_event(value: &Value, event_type: &str) -> Result<AirportEvent> {
    let reason = value
        .get("impactingCondition")
        .or_else(|| value.get("reason"))
        .or_else(|| value.get("text"))
        .or_else(|| value.get("simpleText"))
        .and_then(Value::as_str)
        .map(|value| truncate(value, 120));
    let average_delay_minutes = number_or_numeric_string(value, &["avgDelay", "averageDelay"]);
    let maximum_delay_minutes = number_or_numeric_string(value, &["maxDelay"]);
    let start_time = timestamp_checked(value, &["startTime"])?;
    let end_time = timestamp_checked(value, &["endTime"])?;
    let source_timestamp = timestamp_checked(
        value,
        &[
            "sourceTimeStamp",
            "updateTime",
            "updatedAt",
            "issuedDate",
            "createdAt",
        ],
    )?;
    let advisory_url = value
        .get("advisoryUrl")
        .and_then(Value::as_str)
        .and_then(accepted_faa_advisory_url);
    Ok(AirportEvent {
        event_type: event_type.into(),
        reason,
        average_delay_minutes,
        maximum_delay_minutes,
        start_time,
        end_time,
        source_timestamp,
        advisory_url,
    })
}

fn number_or_numeric_string(value: &Value, fields: &[&str]) -> Option<f64> {
    fields.iter().find_map(|field| {
        value.get(*field).and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
                .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 10_000.0)
        })
    })
}

fn timestamp_checked(value: &Value, fields: &[&str]) -> Result<Option<DateTime<Utc>>> {
    for field in fields {
        let Some(raw) = value.get(*field) else {
            continue;
        };
        if raw.is_null() || raw.as_str().is_some_and(str::is_empty) {
            continue;
        }
        let text = raw
            .as_str()
            .ok_or_else(|| anyhow!("FAA supplied non-text timestamp field {field}"))?;
        let parsed = DateTime::parse_from_rfc3339(text)
            .map_err(|_| anyhow!("FAA supplied malformed timestamp field {field}"))?;
        return Ok(Some(parsed.with_timezone(&Utc)));
    }
    Ok(None)
}

fn accepted_faa_advisory_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    if url.scheme() == "https" && matches!(url.host_str(), Some("www.fly.faa.gov" | "fly.faa.gov"))
    {
        Some(url.into())
    } else {
        None
    }
}

fn truncate(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum FlightCarrier {
    American,
    Delta,
    United,
    Southwest,
    Alaska,
    Jetblue,
}

impl FlightCarrier {
    pub fn label(self) -> &'static str {
        match self {
            Self::American => "American Airlines",
            Self::Delta => "Delta Air Lines",
            Self::United => "United Airlines",
            Self::Southwest => "Southwest Airlines",
            Self::Alaska => "Alaska Airlines",
            Self::Jetblue => "JetBlue",
        }
    }

    pub fn iata_code(self) -> &'static str {
        match self {
            Self::American => "AA",
            Self::Delta => "DL",
            Self::United => "UA",
            Self::Southwest => "WN",
            Self::Alaska => "AS",
            Self::Jetblue => "B6",
        }
    }

    pub fn official_status_page(self) -> &'static str {
        match self {
            Self::American => "https://www.aa.com/travelInformation/flights/status",
            Self::Delta => "https://www.delta.com/flight-status/search",
            Self::United => "https://www.united.com/en/us/flightstatus",
            Self::Southwest => "https://www.southwest.com/air/flight-status/",
            Self::Alaska => "https://www.alaskaair.com/status/day/today",
            Self::Jetblue => "https://www.jetblue.com/flight-tracker-and-status",
        }
    }
}

impl FromStr for FlightCarrier {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "american" | "aa" | "american-airlines" => Ok(Self::American),
            "delta" | "dl" | "delta-air-lines" => Ok(Self::Delta),
            "united" | "ua" | "united-airlines" => Ok(Self::United),
            "southwest" | "wn" | "southwest-airlines" => Ok(Self::Southwest),
            "alaska" | "as" | "alaska-airlines" => Ok(Self::Alaska),
            "jetblue" | "b6" => Ok(Self::Jetblue),
            _ => Err(anyhow!(
                "unsupported carrier; choose American, Delta, United, Southwest, Alaska, or JetBlue explicitly"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FlightStatusHandoff {
    pub schema_version: &'static str,
    pub carrier: &'static str,
    pub flight_identifier: String,
    pub date: Option<String>,
    pub official_status_page: &'static str,
    pub status_retrieved: bool,
    pub network_used: bool,
    pub next_action: &'static str,
    pub surveillance_safeguards: Vec<&'static str>,
    pub limitation: &'static str,
}

pub fn flight_status_handoff(
    carrier: FlightCarrier,
    flight_identifier: &str,
    date: Option<&str>,
) -> Result<FlightStatusHandoff> {
    let flight_identifier = normalize_flight_identifier(carrier, flight_identifier)?;
    let date = date
        .map(|value| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map(|date| date.format("%Y-%m-%d").to_string())
                .map_err(|_| anyhow!("date must use YYYY-MM-DD"))
        })
        .transpose()?;
    let url = Url::parse(carrier.official_status_page())?;
    if url.scheme() != "https" {
        bail!("official airline status page must use HTTPS");
    }
    Ok(FlightStatusHandoff {
        schema_version: "inquiry.flight-handoff/v1",
        carrier: carrier.label(),
        flight_identifier,
        date,
        official_status_page: carrier.official_status_page(),
        status_retrieved: false,
        network_used: false,
        next_action: "Open the official carrier page yourself and enter the normalized flight identifier and date. Inquiry did not contact the carrier.",
        surveillance_safeguards: vec![
            "no live aircraft position or movement history",
            "no owner or passenger association",
            "no background polling, alerts, or bulk lookup",
            "no identifier sent until the user opens and uses the carrier page",
        ],
        limitation: "This is an official-carrier handoff, not a retrieved flight status. For U.S. airport-level traffic-management events, use airport-status; verify operational decisions with the carrier and FAA channels.",
    })
}

fn normalize_flight_identifier(carrier: FlightCarrier, value: &str) -> Result<String> {
    let normalized = value
        .chars()
        .filter(|value| !value.is_ascii_whitespace() && *value != '-')
        .collect::<String>()
        .to_ascii_uppercase();
    if !normalized.is_ascii() {
        bail!("flight identifier may contain only ASCII letters, digits, spaces, or hyphens");
    }
    let suffix = normalized
        .strip_prefix(carrier.iata_code())
        .unwrap_or(&normalized);
    if normalized.len() > suffix.len() && !normalized.starts_with(carrier.iata_code()) {
        bail!("flight identifier prefix does not match the explicitly selected carrier");
    }
    let digit_count = suffix
        .chars()
        .take_while(|value| value.is_ascii_digit())
        .count();
    let trailing = &suffix[digit_count..];
    if !(1..=4).contains(&digit_count)
        || trailing.len() > 1
        || !trailing.chars().all(|value| value.is_ascii_alphabetic())
    {
        bail!("flight identifier must be one to four digits with an optional trailing letter");
    }
    Ok(format!("{}{}", carrier.iata_code(), suffix))
}

#[derive(Debug, Clone, Serialize)]
pub struct AircraftRegistrationLookup {
    pub schema_version: &'static str,
    pub n_number: String,
    pub manufacturer: String,
    pub model: String,
    pub year_manufactured: Option<String>,
    pub aircraft_type: Option<String>,
    pub engine_type: Option<String>,
    pub registration_status: Option<String>,
    pub certificate_issue_date: Option<String>,
    pub expiration_date: Option<String>,
    pub last_activity_date: Option<String>,
    pub dataset: AircraftDatasetProvenance,
    pub omitted_fields: Vec<&'static str>,
    pub surveillance_notice: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct AircraftDatasetProvenance {
    pub archive_filename: String,
    pub archive_size_bytes: u64,
    pub archive_sha256: String,
    pub local_file_modified_at: Option<DateTime<Utc>>,
    pub lookup_at: DateTime<Utc>,
    pub official_download_page: &'static str,
    pub official_schema_documentation: &'static str,
    pub source_note: &'static str,
}

#[derive(Debug)]
struct MasterRecord {
    mfr_model_code: String,
    year_manufactured: Option<String>,
    aircraft_type: Option<String>,
    engine_type: Option<String>,
    status: Option<String>,
    certificate_issue_date: Option<String>,
    expiration_date: Option<String>,
    last_activity_date: Option<String>,
}

#[derive(Debug)]
struct AircraftReferenceRecord {
    manufacturer: String,
    model: String,
    aircraft_type: Option<String>,
    engine_type: Option<String>,
}

pub fn aircraft_registration_lookup(
    archive_path: impl AsRef<Path>,
    n_number: &str,
) -> Result<AircraftRegistrationLookup> {
    let n_number = normalize_n_number(n_number)?;
    let archive_path = archive_path.as_ref();
    if archive_path
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case("zip"))
    {
        bail!("FAA registry archive must be a .zip file downloaded from the official FAA page");
    }
    let mut archive_file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(archive_path)
        .with_context(|| {
            format!(
                "could not open FAA archive {} without following symlinks",
                archive_path.display()
            )
        })?;
    let metadata = archive_file.metadata()?;
    if !metadata.is_file() {
        bail!("FAA registry archive must be a real local file, not a symlink or special file");
    }
    if metadata.len() > 128 * 1024 * 1024 {
        bail!("FAA registry archive exceeded the 128 MiB local safety limit");
    }
    let (archive_snapshot, archive_sha256) = snapshot_archive(&mut archive_file, metadata.len())?;
    let final_metadata = archive_file.metadata()?;
    if final_metadata.len() != metadata.len()
        || final_metadata.modified().ok() != metadata.modified().ok()
    {
        bail!(
            "FAA registry archive changed while Inquiry was reading it; retry with a stable copy"
        );
    }
    let master_entry = find_entry(&archive_snapshot, &["MASTER.TXT"])?;
    let reference_entry = find_entry(&archive_snapshot, &["ACFTREF.TXT"])?;
    let master = find_master_record(&archive_snapshot, &master_entry, &n_number)?
        .ok_or_else(|| anyhow!("FAA archive contained no current registration for {n_number}"))?;
    let reference =
        find_aircraft_reference(&archive_snapshot, &reference_entry, &master.mfr_model_code)?
            .ok_or_else(|| {
                anyhow!(
                    "FAA archive did not contain the aircraft reference code {}",
                    master.mfr_model_code
                )
            })?;
    let local_file_modified_at = metadata.modified().ok().map(DateTime::<Utc>::from);
    Ok(AircraftRegistrationLookup {
        schema_version: "inquiry.aircraft-registration/v1",
        n_number,
        manufacturer: reference.manufacturer,
        model: reference.model,
        year_manufactured: master.year_manufactured,
        aircraft_type: master
            .aircraft_type
            .or(reference.aircraft_type)
            .and_then(|value| aircraft_type_label(&value)),
        engine_type: master
            .engine_type
            .or(reference.engine_type)
            .and_then(|value| engine_type_label(&value)),
        registration_status: master.status.and_then(|value| status_label(&value)),
        certificate_issue_date: master.certificate_issue_date,
        expiration_date: master.expiration_date,
        last_activity_date: master.last_activity_date,
        dataset: AircraftDatasetProvenance {
            archive_filename: archive_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("FAA registry archive")
                .to_owned(),
            archive_size_bytes: metadata.len(),
            archive_sha256,
            local_file_modified_at,
            lookup_at: Utc::now(),
            official_download_page: FAA_REGISTRY_DOWNLOAD_PAGE,
            official_schema_documentation: FAA_REGISTRY_DOCUMENTATION,
            source_note: "The FAA states that its releasable aircraft download is refreshed daily. The SHA-256 identifies the exact local byte snapshot parsed in this lookup; it does not authenticate FAA origin or publication time. The local file timestamp records this copy, not an independently verified FAA publication timestamp.",
        },
        omitted_fields: vec![
            "registrant and co-owner names",
            "street, city, state, postal code, county, and country",
            "aircraft serial number",
            "Mode S transponder code",
            "coordinates and live or historical movement",
            "bulk or reverse-owner enumeration",
        ],
        surveillance_notice: "This single-record registration lookup is not a live flight tracker. Do not combine it with location data to identify a person's movements or pattern of life.",
    })
}

fn normalize_n_number(value: &str) -> Result<String> {
    let upper = value.trim().to_ascii_uppercase();
    let suffix = upper.strip_prefix('N').unwrap_or(&upper);
    if suffix.is_empty()
        || suffix.len() > 5
        || suffix.starts_with('0')
        || !suffix.chars().all(|value| {
            value.is_ascii_digit() || (value.is_ascii_alphabetic() && !matches!(value, 'I' | 'O'))
        })
    {
        bail!("N-number must contain N followed by one to five FAA-compatible letters or digits");
    }
    let first_letter = suffix.chars().position(|value| value.is_ascii_alphabetic());
    if first_letter.is_some_and(|index| suffix[index..].chars().any(|value| value.is_ascii_digit()))
    {
        bail!("N-number letters may appear only at the end");
    }
    if first_letter.is_some_and(|index| suffix.len() - index > 2) {
        bail!("N-number may end with at most two letters");
    }
    Ok(format!("N{suffix}"))
}

fn snapshot_archive(source: &mut File, expected_len: u64) -> Result<(File, String)> {
    let path = std::env::temp_dir().join(format!(
        "barnlabs-inquiry-faa-snapshot-{}.tmp",
        Uuid::new_v4()
    ));
    let mut snapshot = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
        .context("could not create a private FAA archive snapshot")?;
    std::fs::remove_file(&path).context("could not unlink the private FAA archive snapshot")?;

    source.seek(SeekFrom::Start(0))?;
    let mut hasher = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        copied = copied.saturating_add(read as u64);
        if copied > 128 * 1024 * 1024 {
            bail!("FAA registry archive exceeded the 128 MiB local safety limit");
        }
        snapshot.write_all(&buffer[..read])?;
        hasher.update(&buffer[..read]);
    }
    if copied != expected_len {
        bail!(
            "FAA registry archive changed while Inquiry was reading it; retry with a stable copy"
        );
    }
    snapshot.sync_data()?;
    snapshot.seek(SeekFrom::Start(0))?;
    Ok((snapshot, format!("{:x}", hasher.finalize())))
}

fn zip_archive(snapshot: &File) -> Result<ZipArchive<File>> {
    let mut file = snapshot.try_clone()?;
    file.seek(SeekFrom::Start(0))?;
    ZipArchive::new(file).context("FAA registry file was not a valid ZIP archive")
}

fn find_entry(snapshot: &File, expected_names: &[&str]) -> Result<String> {
    let mut archive = zip_archive(snapshot)?;
    if archive.len() > 64 {
        bail!("FAA registry archive contained more than 64 entries");
    }
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        let basename = Path::new(&name)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_uppercase();
        if expected_names.iter().any(|expected| basename == *expected) {
            if entry.size() > 350 * 1024 * 1024 {
                bail!("FAA registry archive entry exceeded the 350 MiB decompressed safety limit");
            }
            if entry.size() > 0
                && (entry.compressed_size() == 0
                    || entry.size() / entry.compressed_size().max(1) > 250)
            {
                bail!("FAA registry archive entry exceeded the compression-ratio safety limit");
            }
            return Ok(name);
        }
    }
    bail!(
        "FAA registry archive omitted required entry {}",
        expected_names.join(" or ")
    )
}

fn find_master_record(
    snapshot: &File,
    entry_name: &str,
    n_number: &str,
) -> Result<Option<MasterRecord>> {
    let mut archive = zip_archive(snapshot)?;
    let entry = archive.by_name(entry_name)?;
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(BufReader::new(RecordBoundedReader::new(entry)));
    let headers = reader.headers()?.clone();
    let n_index = header_index(&headers, &["NNUMBER"])?;
    let model_index = header_index(&headers, &["MFRMDLCODE"])?;
    let year_index = optional_header_index(&headers, &["YEARMFR"]);
    let aircraft_type_index = optional_header_index(&headers, &["TYPEAIRCRAFT", "TYPEACFT"]);
    let engine_type_index = optional_header_index(&headers, &["TYPEENGINE", "TYPEENG"]);
    let status_index = optional_header_index(&headers, &["STATUSCODE", "STATUS"]);
    let certificate_index = optional_header_index(&headers, &["CERTISSUEDATE"]);
    let expiration_index = optional_header_index(&headers, &["EXPIRATIONDATE"]);
    let activity_index = optional_header_index(&headers, &["LASTACTIVITYDATE"]);
    for (count, record) in reader.records().enumerate() {
        if count > 1_000_000 {
            bail!("FAA master file exceeded the 1,000,000-record safety limit");
        }
        let record = record?;
        let record_n = record.get(n_index).unwrap_or("").trim();
        if record_n.eq_ignore_ascii_case(n_number.trim_start_matches('N'))
            || record_n.eq_ignore_ascii_case(n_number)
        {
            return Ok(Some(MasterRecord {
                mfr_model_code: required_cell(&record, model_index, "MFR MDL CODE")?,
                year_manufactured: cell(&record, year_index),
                aircraft_type: cell(&record, aircraft_type_index),
                engine_type: cell(&record, engine_type_index),
                status: cell(&record, status_index),
                certificate_issue_date: cell(&record, certificate_index),
                expiration_date: cell(&record, expiration_index),
                last_activity_date: cell(&record, activity_index),
            }));
        }
    }
    Ok(None)
}

fn find_aircraft_reference(
    snapshot: &File,
    entry_name: &str,
    code: &str,
) -> Result<Option<AircraftReferenceRecord>> {
    let mut archive = zip_archive(snapshot)?;
    let entry = archive.by_name(entry_name)?;
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(BufReader::new(RecordBoundedReader::new(entry)));
    let headers = reader.headers()?.clone();
    let code_index = header_index(&headers, &["CODE", "MFRMDLCODE"])?;
    let manufacturer_index = header_index(&headers, &["MFR", "MANUFACTURER"])?;
    let model_index = header_index(&headers, &["MODEL"])?;
    let aircraft_type_index = optional_header_index(&headers, &["TYPEACFT", "TYPEAIRCRAFT"]);
    let engine_type_index = optional_header_index(&headers, &["TYPEENG", "TYPEENGINE"]);
    for (count, record) in reader.records().enumerate() {
        if count > 1_000_000 {
            bail!("FAA aircraft reference exceeded the 1,000,000-record safety limit");
        }
        let record = record?;
        if record
            .get(code_index)
            .is_some_and(|value| value.trim().eq_ignore_ascii_case(code))
        {
            return Ok(Some(AircraftReferenceRecord {
                manufacturer: required_cell(&record, manufacturer_index, "manufacturer")?,
                model: required_cell(&record, model_index, "model")?,
                aircraft_type: cell(&record, aircraft_type_index),
                engine_type: cell(&record, engine_type_index),
            }));
        }
    }
    Ok(None)
}

fn normalize_header(value: &str) -> String {
    value
        .chars()
        .filter(|value| value.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

fn header_index(headers: &csv::StringRecord, names: &[&str]) -> Result<usize> {
    optional_header_index(headers, names).ok_or_else(|| {
        anyhow!(
            "FAA archive schema omitted required column {}",
            names.join(" or ")
        )
    })
}

fn optional_header_index(headers: &csv::StringRecord, names: &[&str]) -> Option<usize> {
    headers.iter().position(|header| {
        let normalized = normalize_header(header);
        names.iter().any(|name| normalized == *name)
    })
}

fn required_cell(record: &csv::StringRecord, index: usize, field: &str) -> Result<String> {
    let value = record
        .get(index)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("FAA record omitted required {field}"))?;
    if value.chars().count() > 120 {
        bail!("FAA record exceeded the 120-character limit for {field}");
    }
    Ok(value.to_owned())
}

struct RecordBoundedReader<R> {
    inner: R,
    current_record_bytes: usize,
}

impl<R> RecordBoundedReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            current_record_bytes: 0,
        }
    }
}

impl<R: Read> Read for RecordBoundedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        for byte in &buffer[..read] {
            if *byte == b'\n' {
                self.current_record_bytes = 0;
            } else {
                self.current_record_bytes = self.current_record_bytes.saturating_add(1);
                if self.current_record_bytes > 64 * 1024 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "FAA CSV record exceeded the 64 KiB safety limit",
                    ));
                }
            }
        }
        Ok(read)
    }
}

fn cell(record: &csv::StringRecord, index: Option<usize>) -> Option<String> {
    index
        .and_then(|index| record.get(index))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(120).collect())
}

fn aircraft_type_label(code: &str) -> Option<String> {
    Some(
        match code.trim() {
            "1" => "Glider",
            "2" => "Balloon",
            "3" => "Blimp or dirigible",
            "4" => "Fixed-wing single-engine",
            "5" => "Fixed-wing multi-engine",
            "6" => "Rotorcraft",
            "7" => "Weight-shift-control",
            "8" => "Powered parachute",
            "9" => "Gyroplane",
            "H" => "Hybrid lift",
            "O" => "Other",
            _ => return None,
        }
        .into(),
    )
}

fn engine_type_label(code: &str) -> Option<String> {
    Some(
        match code.trim() {
            "0" => "None",
            "1" => "Reciprocating",
            "2" => "Turboprop",
            "3" => "Turboshaft",
            "4" => "Turbojet",
            "5" => "Turbofan",
            "6" => "Ramjet",
            "7" => "Two-cycle",
            "8" => "Four-cycle",
            "9" => "Unknown",
            "10" => "Electric",
            "11" => "Rotary",
            _ => return None,
        }
        .into(),
    )
}

fn status_label(code: &str) -> Option<String> {
    Some(
        match code.trim() {
            "M" => "Valid — manufacturer dealer certificate",
            "R" => "Registration pending",
            "T" => "Valid trainee registration",
            "V" => "Valid registration",
            "W" => "Registration ineffective or invalid",
            "E" => "Registration revoked by enforcement action",
            "6" => "Administratively canceled",
            "7" => "Sale reported",
            "9" => "Registration revoked",
            _ => return None,
        }
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn airport_event_parser_preserves_source_time_and_no_status_overclaim() {
        let value: Value = serde_json::from_str(
            r#"[
              {"airportId":"ATL","airportLongName":"Hartsfield-Jackson Atlanta International","groundDelay":{"impactingCondition":"weather","avgDelay":35,"maxDelay":91,"sourceTimeStamp":"2026-07-17T22:04:04Z","startTime":"2026-07-17T22:00:00Z","endTime":"2026-07-18T02:59:00Z","advisoryUrl":"https://www.fly.faa.gov/adv/one"}},
              {"airportId":"JFK","airportConfig":{"sourceTimeStamp":"2026-07-17T21:00:00Z"}}
            ]"#,
        )
        .unwrap();
        let retrieved = DateTime::parse_from_rfc3339("2026-07-17T22:05:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let atl = parse_airport_events(value.clone(), "ATL", retrieved, 42).unwrap();
        assert_eq!(atl.active_events.len(), 1);
        assert_eq!(atl.active_events[0].average_delay_minutes, Some(35.0));
        assert_eq!(
            atl.source_updated_at.to_rfc3339(),
            "2026-07-17T22:04:04+00:00"
        );
        let jfk = parse_airport_events(value.clone(), "JFK", retrieved, 42).unwrap();
        assert!(jfk.active_events.is_empty());
        assert_eq!(
            jfk.source_updated_at.to_rfc3339(),
            "2026-07-17T21:00:00+00:00"
        );
        assert!(
            jfk.status_statement
                .contains("does not prove normal operations")
        );
        assert!(parse_airport_events(value, "LAX", retrieved, 42).is_err());
    }

    #[test]
    fn airport_event_parser_rejects_stale_empty_and_future_dated_snapshots() {
        let retrieved = DateTime::parse_from_rfc3339("2026-07-17T22:05:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let stale: Value = serde_json::from_str(
            r#"[{"airportId":"JFK","airportConfig":{"sourceTimeStamp":"2026-07-16T21:00:00Z"}}]"#,
        )
        .unwrap();
        assert!(parse_airport_events(stale, "JFK", retrieved, 42).is_err());

        let future: Value = serde_json::from_str(
            r#"[{"airportId":"JFK","airportConfig":{"sourceTimeStamp":"2026-07-17T22:11:00Z"}}]"#,
        )
        .unwrap();
        assert!(parse_airport_events(future, "JFK", retrieved, 42).is_err());

        let stale_event: Value = serde_json::from_str(
            r#"[{"airportId":"JFK","groundDelay":{"sourceTimeStamp":"2026-07-16T20:00:00Z","startTime":"2026-07-16T19:00:00Z","impactingCondition":"weather"}}]"#,
        )
        .unwrap();
        assert!(parse_airport_events(stale_event, "JFK", retrieved, 42).is_err());

        let malformed_event: Value = serde_json::from_str(
            r#"[{"airportId":"JFK","groundDelay":{"sourceTimeStamp":"2026-07-17T22:00:00Z","endTime":"not-a-time"}}]"#,
        )
        .unwrap();
        assert!(parse_airport_events(malformed_event, "JFK", retrieved, 42).is_err());
    }

    #[test]
    fn airport_inputs_and_advisory_redirects_fail_closed() {
        assert!(normalize_airport_id("KJFK").is_err());
        assert!(normalize_airport_id("12A").is_err());
        assert!(accepted_faa_advisory_url("https://evil.example/adv").is_none());
        assert!(accepted_faa_advisory_url("http://www.fly.faa.gov/adv").is_none());
        assert!(rate_limit_message(Some("120")).contains("Retry-After 120"));
    }

    #[test]
    fn flight_handoff_is_exact_and_never_claims_status() {
        let result =
            flight_status_handoff(FlightCarrier::American, "AA 123", Some("2026-07-18")).unwrap();
        assert_eq!(result.flight_identifier, "AA123");
        assert!(!result.status_retrieved);
        assert!(!result.network_used);
        assert!(flight_status_handoff(FlightCarrier::Delta, "AA123", None).is_err());
    }

    #[test]
    fn n_number_validation_rejects_bulk_and_invalid_forms() {
        assert_eq!(normalize_n_number("n123ab").unwrap(), "N123AB");
        assert!(normalize_n_number("N0123").is_err());
        assert!(normalize_n_number("N12A3").is_err());
        assert!(normalize_n_number("N*").is_err());
    }

    #[test]
    fn local_faa_lookup_omits_owner_and_tracking_fields() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ReleasableAircraft.zip");
        let file = File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("MASTER.txt", options).unwrap();
        zip.write_all(b"N-NUMBER,SERIAL NUMBER,MFR MDL CODE,YEAR MFR,TYPE AIRCRAFT,TYPE ENGINE,STATUS CODE,CERT ISSUE DATE,EXPIRATION DATE,LAST ACTIVITY DATE,NAME,STREET1,MODE S CODE HEX\n123AB,SECRET-SERIAL,ABC1234,2020,4,1,V,20200101,20270101,20260701,PRIVATE PERSON,123 PRIVATE STREET,A00001\n").unwrap();
        zip.start_file("ACFTREF.txt", options).unwrap();
        zip.write_all(b"CODE,MFR,MODEL,TYPE-ACFT,TYPE-ENG\nABC1234,BARN AIRCRAFT,MODEL ONE,4,1\n")
            .unwrap();
        zip.finish().unwrap();

        let result = aircraft_registration_lookup(&path, "N123AB").unwrap();
        assert_eq!(result.manufacturer, "BARN AIRCRAFT");
        assert_eq!(result.model, "MODEL ONE");
        assert_eq!(
            result.registration_status.as_deref(),
            Some("Valid registration")
        );
        let serialized = serde_json::to_string(&result).unwrap();
        for forbidden in [
            "PRIVATE PERSON",
            "123 PRIVATE STREET",
            "SECRET-SERIAL",
            "A00001",
        ] {
            assert!(!serialized.contains(forbidden));
        }
        assert!(serialized.contains("registrant and co-owner names"));
    }

    #[test]
    fn local_faa_lookup_rejects_oversized_records_and_required_cells() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("oversized.zip");
        let file = File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("MASTER.txt", options).unwrap();
        let huge = "X".repeat(65 * 1024);
        zip.write_all(format!("N-NUMBER,MFR MDL CODE\n123AB,{huge}\n").as_bytes())
            .unwrap();
        zip.start_file("ACFTREF.txt", options).unwrap();
        zip.write_all(b"CODE,MFR,MODEL\nABC1234,BARN,MODEL\n")
            .unwrap();
        zip.finish().unwrap();
        let error = aircraft_registration_lookup(&path, "N123AB").unwrap_err();
        assert!(
            format!("{error:#}").contains("64 KiB"),
            "unexpected error: {error:#}"
        );

        let path = directory.path().join("long-cell.zip");
        let file = File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("MASTER.txt", options).unwrap();
        let long_code = "A".repeat(121);
        zip.write_all(format!("N-NUMBER,MFR MDL CODE\n123AB,{long_code}\n").as_bytes())
            .unwrap();
        zip.start_file("ACFTREF.txt", options).unwrap();
        zip.write_all(b"CODE,MFR,MODEL\nABC1234,BARN,MODEL\n")
            .unwrap();
        zip.finish().unwrap();
        let error = aircraft_registration_lookup(&path, "N123AB").unwrap_err();
        assert!(error.to_string().contains("120-character"));
    }
}
