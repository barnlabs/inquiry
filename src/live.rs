//! Bounded, on-demand NASA EONET v3 live-event snapshots.
//!
//! This connector intentionally performs one request per explicit call. It does
//! not poll, retry, follow redirects, or fetch any event source link. EONET is a
//! curated natural-event catalog, not an emergency authority or a comprehensive
//! surveillance feed. Geometry timestamps are preserved as provider-supplied
//! timestamps; event existence, extent, impact, and current conditions are not
//! independently verified by Inquiry.

use crate::intent::{IntentKind, resolve as resolve_intent};
use crate::permission::{ConnectorDisclosure, ConnectorRisk, ExecutionPlan, build_execution_plan};
use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Duration, Utc};
use reqwest::header::{HeaderMap, RETRY_AFTER};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::time::{Duration as StdDuration, Instant};
use url::{Host, Url};

pub const EONET_EVENTS_ENDPOINT: &str = "https://eonet.gsfc.nasa.gov/api/v3/events";
pub const EONET_OPEN_EVENTS_URL: &str =
    "https://eonet.gsfc.nasa.gov/api/v3/events?status=open&limit=50";
pub const EONET_DOCUMENTATION_URL: &str = "https://eonet.gsfc.nasa.gov/docs/v3";
pub const EONET_DISCLAIMER_URL: &str = "https://eonet.gsfc.nasa.gov/what-is-eonet";
pub const EONET_CURATION_URL: &str = "https://eonet.gsfc.nasa.gov/event-curation";
pub const LIVE_EVENTS_PLAN_QUERY: &str = "NASA EONET live events";
pub const EONET_CONNECTOR_ID: &str = "nasa-eonet-v3-open-events";

pub const MAX_RESPONSE_BYTES: usize = 1_048_576;
pub const MAX_EVENTS: usize = 50;
pub const MAX_CATEGORIES_PER_EVENT: usize = 8;
pub const MAX_SOURCES_PER_EVENT: usize = 16;
pub const MAX_GEOMETRIES_PER_EVENT: usize = 96;
pub const MAX_POLYGON_RINGS: usize = 32;
pub const MAX_POSITIONS_PER_RING: usize = 2_048;
pub const MAX_POSITIONS_PER_EVENT: usize = 4_096;

const EONET_HOST: &str = "eonet.gsfc.nasa.gov";
const EONET_EVENTS_PATH: &str = "/api/v3/events";
const OPEN_EVENTS_QUERY: &str = "status=open&limit=50";
const SCHEMA_VERSION: &str = "inquiry.live.eonet.v1";
const CONNECT_TIMEOUT: StdDuration = StdDuration::from_secs(8);
const REQUEST_TIMEOUT: StdDuration = StdDuration::from_secs(15);
const FUTURE_TIMESTAMP_TOLERANCE: Duration = Duration::minutes(5);

const MAX_EVENT_ID_CHARS: usize = 96;
const MAX_EVENT_TITLE_CHARS: usize = 240;
const MAX_EVENT_DESCRIPTION_CHARS: usize = 2_000;
const MAX_CATEGORY_ID_CHARS: usize = 120;
const MAX_CATEGORY_TITLE_CHARS: usize = 160;
const MAX_SOURCE_ID_CHARS: usize = 120;
const MAX_URL_CHARS: usize = 2_048;
const MAX_MAGNITUDE_UNIT_CHARS: usize = 64;
const MAX_ENVELOPE_TITLE_CHARS: usize = 160;
const MAX_ENVELOPE_DESCRIPTION_CHARS: usize = 500;
const MAX_ABSOLUTE_MAGNITUDE: f64 = 1.0e15;
const MAX_ABSOLUTE_ALTITUDE: f64 = 100_000.0;

const EONET_LIMITATION: &str = "NASA describes EONET as a curated visualization and general-information resource, not an official source for an event's spatial or temporal extent and not a comprehensive event collection. Confirm consequential claims with competent authorities and the linked primary source.";
const SURVEILLANCE_SAFEGUARD: &str = "This snapshot contains cataloged natural events only. Do not use it for emergency dispatch, navigation, targeting, policing, immigration, insurance, employment, or tracking a person. Inquiry never follows event source links automatically.";
const VERIFICATION_STATEMENT: &str = "Provider-curated by NASA EONET; not independently verified by Inquiry. A provider-open record is not proof that an event is active at retrieval time, and an absent record is not proof that no event exists.";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LiveSnapshot {
    pub schema_version: &'static str,
    pub snapshot_kind: &'static str,
    pub execution_plan_id: String,
    pub approval_mode: LiveApprovalMode,
    pub retrieved_at: DateTime<Utc>,
    pub latest_geometry_source_timestamp: Option<DateTime<Utc>>,
    pub source_age_seconds: Option<i64>,
    pub events: Vec<LiveEvent>,
    pub provenance: LiveProvenance,
    pub operational_limits: LiveOperationalLimits,
    pub provider_rate_limit: ProviderRateLimit,
    pub network_used: bool,
    pub latency_ms: u128,
    pub status_statement: String,
    pub warning: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LiveApprovalMode {
    ExactPlanId,
    AutomaticPublicWeb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedLiveExecution {
    pub plan: ExecutionPlan,
    pub approval_mode: LiveApprovalMode,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LiveEvent {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub eonet_url: String,
    pub provider_status: LiveEventStatus,
    pub closed_at: Option<DateTime<Utc>>,
    pub categories: Vec<LiveCategory>,
    pub sources: Vec<LiveSource>,
    pub geometries: Vec<LiveGeometry>,
    pub verification_status: LiveVerificationStatus,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LiveEventStatus {
    OpenAccordingToEonet,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LiveVerificationStatus {
    ProviderCuratedNotIndependentlyVerified,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LiveCategory {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LiveGeometry {
    pub source_timestamp: DateTime<Utc>,
    pub magnitude: Option<LiveMagnitude>,
    pub shape: LiveGeometryShape,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LiveMagnitude {
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LiveGeometryShape {
    Point { position: LivePosition },
    Polygon { rings: Vec<Vec<LivePosition>> },
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct LivePosition {
    pub longitude: f64,
    pub latitude: f64,
    pub altitude: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LiveSource {
    pub id: String,
    pub url: String,
    pub transport: LiveSourceTransport,
    pub source_timestamp: Option<DateTime<Utc>>,
    pub timestamp_statement: &'static str,
    pub automatically_fetched: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LiveSourceTransport {
    Https,
    HttpLinkOnly,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LiveProvenance {
    pub provider: &'static str,
    pub dataset: &'static str,
    pub endpoint: &'static str,
    pub request_scope: &'static str,
    pub documentation_url: &'static str,
    pub disclaimer_url: &'static str,
    pub curation_url: &'static str,
    pub verification_statement: &'static str,
    pub source_link_policy: &'static str,
    pub operational_notice: &'static str,
    pub surveillance_safeguard: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LiveOperationalLimits {
    pub max_response_bytes: usize,
    pub max_events: usize,
    pub max_categories_per_event: usize,
    pub max_sources_per_event: usize,
    pub max_geometries_per_event: usize,
    pub max_polygon_rings: usize,
    pub max_positions_per_ring: usize,
    pub max_positions_per_event: usize,
    pub network_requests_per_call: u8,
    pub automatic_retries: u8,
    pub background_polling: bool,
    pub redirects_followed: bool,
    pub source_links_fetched: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProviderRateLimit {
    pub limit: Option<u64>,
    pub remaining: Option<u64>,
    pub statement: &'static str,
}

impl Default for ProviderRateLimit {
    fn default() -> Self {
        Self {
            limit: None,
            remaining: None,
            statement: "No valid provider rate-limit headers were supplied; Inquiry does not infer a quota or reset time.",
        }
    }
}

/// Returns true only for the existing, deterministic LiveEvents intent.
pub fn is_live_events_intent(query: &str) -> bool {
    resolve_intent(query).kind == IntentKind::LiveEvents
}

/// Builds the fixed, local-only permission plan for the EONET snapshot.
pub fn eonet_open_execution_plan() -> ExecutionPlan {
    build_execution_plan(
        LIVE_EVENTS_PLAN_QUERY,
        resolve_intent(LIVE_EVENTS_PLAN_QUERY),
        vec![ConnectorDisclosure {
            id: EONET_CONNECTOR_ID.into(),
            service: "NASA EONET v3 public API".into(),
            destinations: vec![EONET_OPEN_EVENTS_URL.into()],
            outbound_data:
                "fixed query parameters only: status=open and limit=50; no user query or identifier"
                    .into(),
            purpose:
                "retrieve one bounded provider-curated natural-event snapshot with source timestamps"
                    .into(),
            risk: ConnectorRisk::PublicQuery,
            automatic_eligible: true,
        }],
    )
}

/// Applies the local permission decision without performing network I/O.
pub fn authorize_eonet_open_snapshot(
    offline: bool,
    approved_plan_id: Option<&str>,
    automatic_public_web: bool,
) -> Result<AuthorizedLiveExecution> {
    if offline {
        bail!("NASA EONET snapshots are unavailable offline; Inquiry did not contact NASA");
    }
    let plan = eonet_open_execution_plan();
    let approval_mode = if approved_plan_id == Some(plan.plan_id.as_str()) {
        LiveApprovalMode::ExactPlanId
    } else if automatic_public_web && plan.automatic_eligible {
        LiveApprovalMode::AutomaticPublicWeb
    } else {
        bail!(
            "NASA EONET connector permission is required before a request can leave the Mac. Inspect `printf 'live events' | inquiry plan --stdin`, then pass --approved-plan {} for this exact plan; --automatic-public-web is accepted only while this fixed public-query plan remains eligible",
            plan.plan_id
        );
    };
    Ok(AuthorizedLiveExecution {
        plan,
        approval_mode,
    })
}

/// Performs exactly one approved GET of NASA EONET's bounded open-event view.
///
/// Authorization is checked before a client is built. `offline` always wins,
/// including when an otherwise valid plan id is supplied. No source link is
/// fetched and no retry or polling task is scheduled.
pub async fn fetch_eonet_open_snapshot(
    offline: bool,
    approved_plan_id: Option<&str>,
    automatic_public_web: bool,
) -> Result<LiveSnapshot> {
    let authorization =
        authorize_eonet_open_snapshot(offline, approved_plan_id, automatic_public_web)?;
    let client = eonet_client()?;
    fetch_eonet_open_snapshot_with_client(&client, authorization).await
}

async fn fetch_eonet_open_snapshot_with_client(
    client: &Client,
    authorization: AuthorizedLiveExecution,
) -> Result<LiveSnapshot> {
    let endpoint = exact_open_events_url()?;
    let started = Instant::now();
    let response = client
        .get(endpoint.clone())
        .send()
        .await
        .context("NASA EONET request failed")?;

    validate_response_destination(&endpoint, response.url())?;
    if response.status().is_redirection() {
        bail!("NASA EONET returned a redirect; Inquiry refused to follow it");
    }
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(sanitize_retry_after);
        bail!(rate_limit_message(retry_after.as_deref()));
    }
    if response.status() == StatusCode::SERVICE_UNAVAILABLE {
        bail!("NASA EONET is temporarily unavailable; Inquiry did not retry automatically");
    }

    let response = response
        .error_for_status()
        .context("NASA EONET rejected the live-event request")?;
    let provider_rate_limit = provider_rate_limit(response.headers());
    // EONET has returned JSON with an incorrect RSS MIME type in production,
    // so the exact destination and bounded JSON parser are authoritative here.
    let body = bytes_limited(response).await?;
    let retrieved_at = Utc::now();
    let events = parse_eonet_open_events(&body, retrieved_at)?;
    Ok(snapshot_from_events(
        events,
        retrieved_at,
        started.elapsed().as_millis(),
        provider_rate_limit,
        authorization.plan.plan_id,
        authorization.approval_mode,
    ))
}

/// Pure parser for deterministic fixtures and network-independent validation.
///
/// `retrieved_at` is supplied by the caller solely to detect impossible future
/// source timestamps and calculate no status by itself. This function does not
/// perform I/O and does not claim that a provider-curated event was verified.
pub fn parse_eonet_open_events(body: &[u8], retrieved_at: DateTime<Utc>) -> Result<Vec<LiveEvent>> {
    if body.len() > MAX_RESPONSE_BYTES {
        bail!("NASA EONET response exceeded the {MAX_RESPONSE_BYTES}-byte safety limit");
    }

    let feed: RawFeed = serde_json::from_slice(body).context("NASA EONET returned invalid JSON")?;
    bounded_text(
        &feed.title,
        "EONET envelope title",
        MAX_ENVELOPE_TITLE_CHARS,
        false,
    )?;
    bounded_text(
        &feed.description,
        "EONET envelope description",
        MAX_ENVELOPE_DESCRIPTION_CHARS,
        true,
    )?;
    if feed.link != EONET_EVENTS_ENDPOINT {
        bail!("NASA EONET response identified an unexpected feed endpoint");
    }
    if feed.events.len() > MAX_EVENTS {
        bail!("NASA EONET response exceeded the {MAX_EVENTS}-event snapshot limit");
    }

    let mut event_ids = HashSet::with_capacity(feed.events.len());
    let mut events = Vec::with_capacity(feed.events.len());
    for raw in feed.events {
        let event = parse_event(raw, retrieved_at)?;
        if !event_ids.insert(event.id.clone()) {
            bail!("NASA EONET response repeated event id {}", event.id);
        }
        events.push(event);
    }
    Ok(events)
}

fn eonet_client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!(
            "BarnLabs-Inquiry/",
            env!("CARGO_PKG_VERSION"),
            " (+https://barnlabs.net/inquiry; on-demand public-data client)"
        ))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("failed to build NASA EONET HTTP client")
}

fn exact_open_events_url() -> Result<Url> {
    let url = Url::parse(EONET_OPEN_EVENTS_URL).context("invalid built-in NASA EONET endpoint")?;
    validate_endpoint(&url)?;
    Ok(url)
}

fn validate_endpoint(url: &Url) -> Result<()> {
    if url.scheme() != "https"
        || url.host_str() != Some(EONET_HOST)
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != EONET_EVENTS_PATH
        || url.query() != Some(OPEN_EVENTS_QUERY)
        || url.fragment().is_some()
    {
        bail!("NASA EONET destination is not on the exact HTTPS allowlist");
    }
    Ok(())
}

fn validate_response_destination(expected: &Url, actual: &Url) -> Result<()> {
    validate_endpoint(actual)?;
    if actual != expected {
        bail!("NASA EONET response did not come from the exact requested endpoint");
    }
    Ok(())
}

async fn bytes_limited(mut response: Response) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        bail!("NASA EONET response exceeded the {MAX_RESPONSE_BYTES}-byte safety limit");
    }

    let mut body = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_RESPONSE_BYTES as u64) as usize,
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .context("NASA EONET response stream failed")?
    {
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            bail!("NASA EONET response exceeded the {MAX_RESPONSE_BYTES}-byte safety limit");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn parse_event(raw: RawEvent, retrieved_at: DateTime<Utc>) -> Result<LiveEvent> {
    let id = bounded_event_id(&raw.id)?;
    let title = bounded_text(
        &raw.title,
        "EONET event title",
        MAX_EVENT_TITLE_CHARS,
        false,
    )?;
    let description = raw
        .description
        .as_deref()
        .map(|value| {
            bounded_text(
                value,
                "EONET event description",
                MAX_EVENT_DESCRIPTION_CHARS,
                true,
            )
        })
        .transpose()?;
    let eonet_url = validate_event_url(&id, &raw.link)?;
    let closed_at = raw
        .closed
        .as_deref()
        .map(|value| parse_source_timestamp(value, "EONET event closed timestamp", retrieved_at))
        .transpose()?;
    if closed_at.is_some() {
        bail!(
            "NASA EONET open-event response marked event {id} closed; Inquiry abstained from treating the response as an open snapshot"
        );
    }
    if raw.categories.is_empty() {
        bail!("NASA EONET event {id} omitted its category");
    }
    if raw.categories.len() > MAX_CATEGORIES_PER_EVENT {
        bail!("NASA EONET event {id} exceeded the {MAX_CATEGORIES_PER_EVENT}-category limit");
    }
    if raw.sources.is_empty() {
        bail!("NASA EONET event {id} omitted its source links");
    }
    if raw.sources.len() > MAX_SOURCES_PER_EVENT {
        bail!("NASA EONET event {id} exceeded the {MAX_SOURCES_PER_EVENT}-source limit");
    }
    if raw.geometry.is_empty() {
        bail!("NASA EONET event {id} omitted its geometry history");
    }
    if raw.geometry.len() > MAX_GEOMETRIES_PER_EVENT {
        bail!("NASA EONET event {id} exceeded the {MAX_GEOMETRIES_PER_EVENT}-geometry limit");
    }

    let categories = raw
        .categories
        .into_iter()
        .map(parse_category)
        .collect::<Result<Vec<_>>>()?;
    let sources = raw
        .sources
        .into_iter()
        .map(parse_source)
        .collect::<Result<Vec<_>>>()?;
    let mut total_positions = 0usize;
    let geometries = raw
        .geometry
        .into_iter()
        .map(|geometry| parse_geometry(geometry, retrieved_at, &mut total_positions))
        .collect::<Result<Vec<_>>>()?;

    Ok(LiveEvent {
        id,
        title,
        description,
        eonet_url,
        provider_status: LiveEventStatus::OpenAccordingToEonet,
        closed_at,
        categories,
        sources,
        geometries,
        verification_status: LiveVerificationStatus::ProviderCuratedNotIndependentlyVerified,
    })
}

fn parse_category(raw: RawCategory) -> Result<LiveCategory> {
    Ok(LiveCategory {
        id: bounded_text(&raw.id, "EONET category id", MAX_CATEGORY_ID_CHARS, false)?,
        title: bounded_text(
            &raw.title,
            "EONET category title",
            MAX_CATEGORY_TITLE_CHARS,
            false,
        )?,
    })
}

fn parse_source(raw: RawSource) -> Result<LiveSource> {
    let id = bounded_text(&raw.id, "EONET source id", MAX_SOURCE_ID_CHARS, false)?;
    let url = bounded_text(&raw.url, "EONET source URL", MAX_URL_CHARS, false)?;
    let parsed = Url::parse(&url).context("NASA EONET source link was not a valid URL")?;
    if parsed.host_str().is_none()
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
        || parsed.host().is_some_and(source_host_is_local)
    {
        bail!("NASA EONET source link was not a safe public web URL");
    }
    let transport = match parsed.scheme() {
        "https" => LiveSourceTransport::Https,
        "http" => LiveSourceTransport::HttpLinkOnly,
        _ => bail!("NASA EONET source link used a non-web URL scheme"),
    };

    Ok(LiveSource {
        id,
        url,
        transport,
        source_timestamp: None,
        timestamp_statement: "EONET v3 does not provide a timestamp for an individual source link; geometry timestamps must not be attributed to the linked publisher.",
        automatically_fetched: false,
    })
}

fn source_host_is_local(host: Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => {
            let domain = domain.trim_end_matches('.').to_ascii_lowercase();
            domain == "localhost"
                || domain.ends_with(".localhost")
                || domain.ends_with(".local")
                || domain.ends_with(".internal")
        }
        Host::Ipv4(address) => {
            address.is_private()
                || address.is_loopback()
                || address.is_link_local()
                || address.is_broadcast()
                || address.is_unspecified()
                || address.is_multicast()
        }
        Host::Ipv6(address) => {
            address.is_loopback()
                || address.is_unspecified()
                || address.is_unique_local()
                || address.is_unicast_link_local()
                || address.is_multicast()
        }
    }
}

fn parse_geometry(
    raw: RawGeometry,
    retrieved_at: DateTime<Utc>,
    total_positions: &mut usize,
) -> Result<LiveGeometry> {
    let source_timestamp =
        parse_source_timestamp(&raw.date, "EONET geometry source timestamp", retrieved_at)?;
    let magnitude = match (raw.magnitude_value, raw.magnitude_unit) {
        (None, None) => None,
        (Some(value), Some(unit)) => {
            if !value.is_finite() || value.abs() > MAX_ABSOLUTE_MAGNITUDE {
                bail!("NASA EONET geometry supplied an invalid magnitude");
            }
            Some(LiveMagnitude {
                value,
                unit: bounded_text(
                    &unit,
                    "EONET magnitude unit",
                    MAX_MAGNITUDE_UNIT_CHARS,
                    false,
                )?,
            })
        }
        _ => bail!("NASA EONET geometry supplied an incomplete magnitude"),
    };

    let shape = match raw.geometry_type.as_str() {
        "Point" => {
            *total_positions = total_positions.saturating_add(1);
            ensure_total_positions(*total_positions)?;
            LiveGeometryShape::Point {
                position: parse_position(&raw.coordinates)?,
            }
        }
        "Polygon" => LiveGeometryShape::Polygon {
            rings: parse_polygon(&raw.coordinates, total_positions)?,
        },
        other => bail!("NASA EONET geometry type {other:?} is outside the bounded contract"),
    };

    Ok(LiveGeometry {
        source_timestamp,
        magnitude,
        shape,
    })
}

fn parse_polygon(value: &Value, total_positions: &mut usize) -> Result<Vec<Vec<LivePosition>>> {
    let raw_rings = value
        .as_array()
        .ok_or_else(|| anyhow!("NASA EONET polygon coordinates were not an array"))?;
    if raw_rings.is_empty() || raw_rings.len() > MAX_POLYGON_RINGS {
        bail!("NASA EONET polygon must contain between 1 and {MAX_POLYGON_RINGS} rings");
    }

    let mut rings = Vec::with_capacity(raw_rings.len());
    for raw_ring in raw_rings {
        let positions = raw_ring
            .as_array()
            .ok_or_else(|| anyhow!("NASA EONET polygon ring was not an array"))?;
        if positions.len() < 4 || positions.len() > MAX_POSITIONS_PER_RING {
            bail!(
                "NASA EONET polygon ring must contain between 4 and {MAX_POSITIONS_PER_RING} positions"
            );
        }
        *total_positions = total_positions.saturating_add(positions.len());
        ensure_total_positions(*total_positions)?;

        let ring = positions
            .iter()
            .map(parse_position)
            .collect::<Result<Vec<_>>>()?;
        let first = ring
            .first()
            .ok_or_else(|| anyhow!("NASA EONET polygon ring was empty"))?;
        let last = ring
            .last()
            .ok_or_else(|| anyhow!("NASA EONET polygon ring was empty"))?;
        if first.longitude != last.longitude
            || first.latitude != last.latitude
            || first.altitude != last.altitude
        {
            bail!("NASA EONET polygon ring was not closed as required by GeoJSON");
        }
        rings.push(ring);
    }
    Ok(rings)
}

fn parse_position(value: &Value) -> Result<LivePosition> {
    let values = value
        .as_array()
        .ok_or_else(|| anyhow!("NASA EONET position was not an array"))?;
    if !(2..=3).contains(&values.len()) {
        bail!("NASA EONET position must contain longitude, latitude, and optional altitude");
    }

    let longitude = finite_number(&values[0], "longitude")?;
    let latitude = finite_number(&values[1], "latitude")?;
    let altitude = values
        .get(2)
        .map(|value| finite_number(value, "altitude"))
        .transpose()?;
    if !(-180.0..=180.0).contains(&longitude) {
        bail!("NASA EONET longitude was outside -180 through 180 degrees");
    }
    if !(-90.0..=90.0).contains(&latitude) {
        bail!("NASA EONET latitude was outside -90 through 90 degrees");
    }
    if altitude.is_some_and(|value| value.abs() > MAX_ABSOLUTE_ALTITUDE) {
        bail!("NASA EONET altitude was outside the bounded contract");
    }

    Ok(LivePosition {
        longitude,
        latitude,
        altitude,
    })
}

fn finite_number(value: &Value, label: &str) -> Result<f64> {
    let number = value
        .as_f64()
        .ok_or_else(|| anyhow!("NASA EONET {label} was not a number"))?;
    if !number.is_finite() {
        bail!("NASA EONET {label} was not finite");
    }
    Ok(number)
}

fn ensure_total_positions(total_positions: usize) -> Result<()> {
    if total_positions > MAX_POSITIONS_PER_EVENT {
        bail!("NASA EONET event exceeded the {MAX_POSITIONS_PER_EVENT}-position geometry limit");
    }
    Ok(())
}

fn parse_source_timestamp(
    value: &str,
    label: &str,
    retrieved_at: DateTime<Utc>,
) -> Result<DateTime<Utc>> {
    let timestamp = DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("{label} was not RFC 3339"))?
        .with_timezone(&Utc);
    if timestamp > retrieved_at + FUTURE_TIMESTAMP_TOLERANCE {
        bail!("{label} was more than five minutes in the future");
    }
    Ok(timestamp)
}

fn bounded_event_id(value: &str) -> Result<String> {
    let id = bounded_text(value, "EONET event id", MAX_EVENT_ID_CHARS, false)?;
    if !id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        bail!("NASA EONET event id contained unsupported characters");
    }
    Ok(id)
}

fn bounded_text(value: &str, label: &str, max_chars: usize, multiline: bool) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("{label} was empty");
    }
    if trimmed.chars().count() > max_chars {
        bail!("{label} exceeded the {max_chars}-character limit");
    }
    if trimmed.chars().any(|character| {
        character.is_control() && !(multiline && matches!(character, '\n' | '\r' | '\t'))
    }) {
        bail!("{label} contained unsupported control characters");
    }
    Ok(trimmed.to_owned())
}

fn validate_event_url(event_id: &str, value: &str) -> Result<String> {
    let value = bounded_text(value, "EONET event URL", MAX_URL_CHARS, false)?;
    let url = Url::parse(&value).context("NASA EONET event URL was invalid")?;
    let expected_path = format!("{EONET_EVENTS_PATH}/{event_id}");
    if url.scheme() != "https"
        || url.host_str() != Some(EONET_HOST)
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != expected_path
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("NASA EONET event URL was outside the exact event-link allowlist");
    }
    Ok(value)
}

fn snapshot_from_events(
    events: Vec<LiveEvent>,
    retrieved_at: DateTime<Utc>,
    latency_ms: u128,
    provider_rate_limit: ProviderRateLimit,
    execution_plan_id: String,
    approval_mode: LiveApprovalMode,
) -> LiveSnapshot {
    let latest_geometry_source_timestamp = events
        .iter()
        .flat_map(|event| event.geometries.iter())
        .map(|geometry| geometry.source_timestamp)
        .max();
    let source_age_seconds = latest_geometry_source_timestamp.map(|timestamp| {
        retrieved_at
            .signed_duration_since(timestamp)
            .num_seconds()
            .max(0)
    });
    let status_statement = match source_age_seconds {
        Some(age) => format!(
            "NASA EONET returned {} provider-curated open event records. The newest geometry timestamp was {age} seconds before retrieval; Inquiry did not independently verify any event.",
            events.len()
        ),
        None => format!(
            "NASA EONET returned {} provider-curated open event records and no geometry timestamp. Inquiry does not treat an empty snapshot as proof that no event exists.",
            events.len()
        ),
    };

    LiveSnapshot {
        schema_version: SCHEMA_VERSION,
        snapshot_kind: "nasa_eonet_open_natural_events",
        execution_plan_id,
        approval_mode,
        retrieved_at,
        latest_geometry_source_timestamp,
        source_age_seconds,
        events,
        provenance: LiveProvenance {
            provider: "NASA Earth Observatory Natural Event Tracker (EONET)",
            dataset: "EONET v3 open events",
            endpoint: EONET_OPEN_EVENTS_URL,
            request_scope: "status=open; limit=50; provider order preserved",
            documentation_url: EONET_DOCUMENTATION_URL,
            disclaimer_url: EONET_DISCLAIMER_URL,
            curation_url: EONET_CURATION_URL,
            verification_statement: VERIFICATION_STATEMENT,
            source_link_policy: "Source links are provenance pointers only. They are validated as public HTTP(S) URLs, never fetched by this connector, and carry no EONET per-link timestamp.",
            operational_notice: EONET_LIMITATION,
            surveillance_safeguard: SURVEILLANCE_SAFEGUARD,
        },
        operational_limits: LiveOperationalLimits {
            max_response_bytes: MAX_RESPONSE_BYTES,
            max_events: MAX_EVENTS,
            max_categories_per_event: MAX_CATEGORIES_PER_EVENT,
            max_sources_per_event: MAX_SOURCES_PER_EVENT,
            max_geometries_per_event: MAX_GEOMETRIES_PER_EVENT,
            max_polygon_rings: MAX_POLYGON_RINGS,
            max_positions_per_ring: MAX_POSITIONS_PER_RING,
            max_positions_per_event: MAX_POSITIONS_PER_EVENT,
            network_requests_per_call: 1,
            automatic_retries: 0,
            background_polling: false,
            redirects_followed: false,
            source_links_fetched: false,
        },
        provider_rate_limit,
        network_used: true,
        latency_ms,
        status_statement,
        warning: EONET_LIMITATION,
    }
}

fn provider_rate_limit(headers: &HeaderMap) -> ProviderRateLimit {
    ProviderRateLimit {
        limit: bounded_u64_header(headers, "x-ratelimit-limit"),
        remaining: bounded_u64_header(headers, "x-ratelimit-remaining"),
        statement: "Values are copied from this response's provider headers when valid; Inquiry does not infer a reset time or poll for quota changes.",
    }
}

fn bounded_u64_header(headers: &HeaderMap, name: &str) -> Option<u64> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.len() <= 20 && value.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|value| value.parse().ok())
}

fn sanitize_retry_after(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 64 || value.chars().any(char::is_control) {
        return None;
    }
    Some(value.to_owned())
}

fn rate_limit_message(retry_after: Option<&str>) -> String {
    match retry_after {
        Some(value) => format!(
            "NASA EONET rate limit reached; do not retry before provider Retry-After {value:?}. Inquiry did not retry automatically"
        ),
        None => {
            "NASA EONET rate limit reached; Inquiry did not retry automatically because no usable Retry-After value was supplied".into()
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawFeed {
    title: String,
    description: String,
    link: String,
    events: Vec<RawEvent>,
}

#[derive(Debug, Deserialize)]
struct RawEvent {
    id: String,
    title: String,
    description: Option<String>,
    link: String,
    closed: Option<String>,
    categories: Vec<RawCategory>,
    sources: Vec<RawSource>,
    geometry: Vec<RawGeometry>,
}

#[derive(Debug, Deserialize)]
struct RawCategory {
    id: String,
    title: String,
}

#[derive(Debug, Deserialize)]
struct RawSource {
    id: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct RawGeometry {
    #[serde(rename = "magnitudeValue")]
    magnitude_value: Option<f64>,
    #[serde(rename = "magnitudeUnit")]
    magnitude_unit: Option<String>,
    date: String,
    #[serde(rename = "type")]
    geometry_type: String,
    coordinates: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FIXTURE: &[u8] = br#"
    {
      "title": "EONET Events",
      "description": "Natural events from EONET.",
      "link": "https://eonet.gsfc.nasa.gov/api/v3/events",
      "events": [
        {
          "id": "EONET_9999",
          "title": "Fixture wildfire",
          "description": "A deterministic parser fixture, not a real event.",
          "link": "https://eonet.gsfc.nasa.gov/api/v3/events/EONET_9999",
          "closed": null,
          "categories": [{"id": "wildfires", "title": "Wildfires"}],
          "sources": [
            {"id": "HTTPS_SOURCE", "url": "https://example.gov/events/9999"},
            {"id": "HTTP_SOURCE", "url": "http://example.org/archive/9999"}
          ],
          "geometry": [
            {
              "magnitudeValue": 1250.5,
              "magnitudeUnit": "acres",
              "date": "2026-07-16T08:04:00Z",
              "type": "Point",
              "coordinates": [-120.24295, 45.336333]
            },
            {
              "magnitudeValue": null,
              "magnitudeUnit": null,
              "date": "2026-07-16T09:00:00Z",
              "type": "Polygon",
              "coordinates": [[
                [-120.0, 45.0],
                [-119.9, 45.0],
                [-119.9, 45.1],
                [-120.0, 45.0]
              ]]
            }
          ]
        }
      ]
    }
    "#;

    fn retrieved_at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-07-17T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn fixture_value() -> Value {
        serde_json::from_slice(FIXTURE).unwrap()
    }

    #[test]
    fn parses_typed_bounded_fixture_without_inventing_verification() {
        let events = parse_eonet_open_events(FIXTURE, retrieved_at()).unwrap();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.id, "EONET_9999");
        assert_eq!(event.provider_status, LiveEventStatus::OpenAccordingToEonet);
        assert_eq!(
            event.verification_status,
            LiveVerificationStatus::ProviderCuratedNotIndependentlyVerified
        );
        assert_eq!(event.categories[0].id, "wildfires");
        assert_eq!(event.sources[0].transport, LiveSourceTransport::Https);
        assert_eq!(
            event.sources[1].transport,
            LiveSourceTransport::HttpLinkOnly
        );
        assert!(
            event.sources.iter().all(|source| {
                source.source_timestamp.is_none() && !source.automatically_fetched
            })
        );
        assert_eq!(
            event.geometries[1].source_timestamp,
            DateTime::parse_from_rfc3339("2026-07-16T09:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );
        assert!(matches!(
            event.geometries[0].shape,
            LiveGeometryShape::Point { .. }
        ));
        assert!(matches!(
            event.geometries[1].shape,
            LiveGeometryShape::Polygon { .. }
        ));

        let snapshot = snapshot_from_events(
            events,
            retrieved_at(),
            17,
            ProviderRateLimit::default(),
            "sha256:fixture".into(),
            LiveApprovalMode::ExactPlanId,
        );
        assert!(!snapshot.operational_limits.background_polling);
        assert_eq!(snapshot.operational_limits.automatic_retries, 0);
        assert!(!snapshot.operational_limits.source_links_fetched);
        assert!(
            snapshot
                .status_statement
                .contains("did not independently verify")
        );
        let serialized = serde_json::to_value(snapshot).unwrap();
        assert_eq!(serialized["execution_plan_id"], "sha256:fixture");
        assert_eq!(serialized["approval_mode"], "exact_plan_id");
        assert!(serialized.get("executionPlanId").is_none());
    }

    #[test]
    fn live_plan_is_deterministic_exact_and_automatically_eligible() {
        assert!(is_live_events_intent("show live events in real time"));
        assert!(is_live_events_intent("NASA EONET"));
        let first = eonet_open_execution_plan();
        let second = eonet_open_execution_plan();
        assert_eq!(first, second);
        assert!(first.permission_required);
        assert!(first.automatic_eligible);
        assert_eq!(first.connectors.len(), 1);
        assert_eq!(first.connectors[0].id, EONET_CONNECTOR_ID);
        assert_eq!(
            first.connectors[0].destinations,
            vec![EONET_OPEN_EVENTS_URL]
        );
        assert!(
            first.connectors[0]
                .outbound_data
                .contains("no user query or identifier")
        );
    }

    #[test]
    fn authorization_requires_exact_plan_or_eligible_automatic_mode_and_offline_wins() {
        let plan = eonet_open_execution_plan();
        let exact = authorize_eonet_open_snapshot(false, Some(&plan.plan_id), false).unwrap();
        assert_eq!(exact.approval_mode, LiveApprovalMode::ExactPlanId);
        assert_eq!(exact.plan.plan_id, plan.plan_id);

        let automatic = authorize_eonet_open_snapshot(false, None, true).unwrap();
        assert_eq!(
            automatic.approval_mode,
            LiveApprovalMode::AutomaticPublicWeb
        );

        for rejected in [None, Some("sha256:not-the-plan")] {
            let error = authorize_eonet_open_snapshot(false, rejected, false)
                .unwrap_err()
                .to_string();
            assert!(error.contains(&plan.plan_id));
        }

        let error = authorize_eonet_open_snapshot(true, Some(&plan.plan_id), true)
            .unwrap_err()
            .to_string();
        assert!(error.contains("unavailable offline"));
    }

    #[tokio::test]
    async fn offline_fetch_fails_before_building_or_sending_a_request() {
        let error = fetch_eonet_open_snapshot(true, None, false)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("did not contact NASA"));
    }

    #[test]
    fn rejects_body_over_response_limit_before_json_parsing() {
        let body = vec![b' '; MAX_RESPONSE_BYTES + 1];
        let error = parse_eonet_open_events(&body, retrieved_at())
            .unwrap_err()
            .to_string();
        assert!(error.contains("byte safety limit"));
    }

    #[test]
    fn rejects_more_events_than_the_fixed_request_contract() {
        let mut value = fixture_value();
        let event = value["events"][0].clone();
        value["events"] = Value::Array(vec![event; MAX_EVENTS + 1]);
        let body = serde_json::to_vec(&value).unwrap();
        let error = parse_eonet_open_events(&body, retrieved_at())
            .unwrap_err()
            .to_string();
        assert!(error.contains("event snapshot limit"));
    }

    #[test]
    fn rejects_geometry_position_exhaustion() {
        let mut value = fixture_value();
        let mut ring = Vec::with_capacity(MAX_POSITIONS_PER_RING + 1);
        for index in 0..=MAX_POSITIONS_PER_RING {
            ring.push(json!([-120.0 + (index as f64 / 100_000.0), 45.0]));
        }
        value["events"][0]["geometry"][1]["coordinates"] = json!([ring]);
        let body = serde_json::to_vec(&value).unwrap();
        let error = parse_eonet_open_events(&body, retrieved_at())
            .unwrap_err()
            .to_string();
        assert!(error.contains("polygon ring must contain"));
    }

    #[test]
    fn rejects_long_strings_invalid_coordinates_and_bad_timestamps() {
        let mut long_title = fixture_value();
        long_title["events"][0]["title"] = json!("x".repeat(MAX_EVENT_TITLE_CHARS + 1));
        let error =
            parse_eonet_open_events(&serde_json::to_vec(&long_title).unwrap(), retrieved_at())
                .unwrap_err()
                .to_string();
        assert!(error.contains("event title"));

        let mut invalid_coordinate = fixture_value();
        invalid_coordinate["events"][0]["geometry"][0]["coordinates"] = json!([-190.0, 45.0]);
        let error = parse_eonet_open_events(
            &serde_json::to_vec(&invalid_coordinate).unwrap(),
            retrieved_at(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("longitude"));

        let mut bad_timestamp = fixture_value();
        bad_timestamp["events"][0]["geometry"][0]["date"] = json!("tomorrow-ish");
        let error =
            parse_eonet_open_events(&serde_json::to_vec(&bad_timestamp).unwrap(), retrieved_at())
                .unwrap_err()
                .to_string();
        assert!(error.contains("RFC 3339"));
    }

    #[test]
    fn source_links_are_click_only_and_cannot_name_local_services() {
        let mut local_source = fixture_value();
        local_source["events"][0]["sources"][0]["url"] = json!("http://127.0.0.1/private-admin");
        let error =
            parse_eonet_open_events(&serde_json::to_vec(&local_source).unwrap(), retrieved_at())
                .unwrap_err()
                .to_string();
        assert!(error.contains("safe public web URL"));
    }

    #[test]
    fn rejects_closed_event_from_open_endpoint_and_future_geometry() {
        let mut closed = fixture_value();
        closed["events"][0]["closed"] = json!("2026-07-17T10:00:00Z");
        let error = parse_eonet_open_events(&serde_json::to_vec(&closed).unwrap(), retrieved_at())
            .unwrap_err()
            .to_string();
        assert!(error.contains("marked event EONET_9999 closed"));

        let mut future = fixture_value();
        future["events"][0]["geometry"][0]["date"] = json!("2026-07-17T12:06:00Z");
        let error = parse_eonet_open_events(&serde_json::to_vec(&future).unwrap(), retrieved_at())
            .unwrap_err()
            .to_string();
        assert!(error.contains("more than five minutes in the future"));
    }

    #[test]
    fn exact_endpoint_policy_rejects_host_scheme_path_query_and_redirect_variants() {
        let expected = exact_open_events_url().unwrap();
        assert!(validate_endpoint(&expected).is_ok());

        for rejected in [
            "http://eonet.gsfc.nasa.gov/api/v3/events?status=open&limit=50",
            "https://example.com/api/v3/events?status=open&limit=50",
            "https://eonet.gsfc.nasa.gov/api/v2/events?status=open&limit=50",
            "https://eonet.gsfc.nasa.gov/api/v3/events?status=all&limit=50",
            "https://eonet.gsfc.nasa.gov/api/v3/events?status=open&limit=50&days=1",
        ] {
            assert!(validate_endpoint(&Url::parse(rejected).unwrap()).is_err());
        }

        let altered =
            Url::parse("https://eonet.gsfc.nasa.gov/api/v3/events?limit=50&status=open").unwrap();
        assert!(validate_response_destination(&expected, &altered).is_err());
    }

    #[test]
    fn retry_after_is_bounded_and_never_causes_an_automatic_retry() {
        assert_eq!(sanitize_retry_after(" 120 ").as_deref(), Some("120"));
        assert_eq!(
            sanitize_retry_after("Sat, 18 Jul 2026 01:35:00 GMT").as_deref(),
            Some("Sat, 18 Jul 2026 01:35:00 GMT")
        );
        assert!(sanitize_retry_after("bad\nvalue").is_none());
        assert!(sanitize_retry_after(&"x".repeat(65)).is_none());
        assert!(rate_limit_message(Some("120")).contains("did not retry automatically"));
    }
}
