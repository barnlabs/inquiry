use crate::http::{bytes_limited, json_limited};
use crate::intent::{IntentKind, resolve as resolve_intent};
use crate::model::{
    Confidence, ConnectorAudit, ContentTrust, Facet, Finding, Metric, ProvenanceDetails,
    ResearchPlan, SourceOutput, SourceQuality, SourceRecord, SourceType,
};
use crate::permission::{ConnectorDisclosure, ConnectorRisk};
use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use chrono::{Datelike, Utc};
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};
use regex::Regex;
use reqwest::Client;
use reqwest::header::RETRY_AFTER;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use url::Url;

#[async_trait]
pub trait PublicSource: Send + Sync {
    fn name(&self) -> &'static str;
    fn supports(&self, plan: &ResearchPlan) -> bool;
    fn disclosures(&self, plan: &ResearchPlan) -> Vec<ConnectorDisclosure> {
        disclosures_for_source(self.name(), plan)
    }
    async fn search(&self, plan: &ResearchPlan, limit: usize) -> Result<SourceOutput>;
}

fn disclosure(
    id: &str,
    service: &str,
    destinations: &[&str],
    outbound_data: &str,
    purpose: &str,
) -> ConnectorDisclosure {
    ConnectorDisclosure {
        id: id.into(),
        service: service.into(),
        destinations: destinations.iter().map(|value| (*value).into()).collect(),
        outbound_data: outbound_data.into(),
        purpose: purpose.into(),
        risk: ConnectorRisk::PublicQuery,
        automatic_eligible: true,
    }
}

fn disclosures_for_source(name: &str, plan: &ResearchPlan) -> Vec<ConnectorDisclosure> {
    match name {
        "Wikidata current officeholder" => {
            let mut values = vec![
                disclosure(
                    "wikidata-office",
                    "Wikidata",
                    &["www.wikidata.org"],
                    "the public office and jurisdiction query",
                    "resolve an exact structured office statement",
                ),
                disclosure(
                    "wikipedia-biography",
                    "Wikipedia",
                    &["en.wikipedia.org"],
                    "the resolved public biography title",
                    "retrieve a concise biography summary",
                ),
                disclosure(
                    "commons-portrait",
                    "Wikimedia Commons",
                    &["commons.wikimedia.org", "upload.wikimedia.org"],
                    "the resolved public Wikidata entity and portrait filename; an eligible preview request also reveals IP address, request time, and image URL",
                    "retrieve file-specific portrait metadata, reuse terms, and an approved bounded preview",
                ),
            ];
            match official_current_office(&plan.query) {
                Some(OfficialCurrentOffice::UnitedStatesPresident) => {
                    values.push(disclosure(
                        "usagov-current-president",
                        "USAGov",
                        &["www.usa.gov"],
                        "no query parameters; Inquiry retrieves the public presidents page",
                        "corroborate the current office identity",
                    ));
                    values.push(disclosure(
                        "white-house-biography",
                        "The White House",
                        &["www.whitehouse.gov"],
                        "the official biography URL exposed by USAGov",
                        "retrieve an institution-controlled biography when linked",
                    ));
                }
                Some(OfficialCurrentOffice::UnitedKingdomMonarch) => {
                    values.push(disclosure(
                        "uk-parliament-reign",
                        "UK Parliament",
                        &["api.parliament.uk"],
                        "no query parameters; Inquiry retrieves current reign tables",
                        "corroborate the current monarch identity and reign",
                    ));
                    values.push(disclosure(
                        "royal-family-profile",
                        "The Royal Family",
                        &["www.royal.uk"],
                        "the resolved public monarch identity",
                        "corroborate identity and retrieve the official biography",
                    ));
                }
                None => {}
            }
            values
        }
        "Wikipedia" => vec![disclosure(
            "wikipedia-search",
            "Wikipedia",
            &["en.wikipedia.org"],
            "the minimized public research query",
            "retrieve encyclopedia discovery records",
        )],
        "GDELT DOC" => vec![disclosure(
            "gdelt-doc",
            "GDELT DOC 2.0",
            &["api.gdeltproject.org"],
            "a minimized public event query and a rolling three-month time window",
            "discover recent news articles and image candidates for event resolution",
        )],
        "Wikimedia Commons media" => vec![disclosure(
            "commons-media-search",
            "Wikimedia Commons",
            &["commons.wikimedia.org", "upload.wikimedia.org"],
            "the minimized public media query and, for recent events, at most one public event-title refinement derived from an accepted discovery record; an eligible preview request also reveals IP address, request time, and image URL",
            "discover media, retain only accepted file-specific rights metadata, and provide an approved bounded preview",
        )],
        "NASA 3D Resources" => vec![disclosure(
            "nasa-3d",
            "NASA 3D Resources",
            &["science.nasa.gov"],
            "no query parameters; filtering occurs locally after retrieval",
            "discover public NASA 3D assets",
        )],
        "MedlinePlus" => vec![disclosure(
            "medlineplus",
            "MedlinePlus",
            &["wsearch.nlm.nih.gov"],
            "a minimized public health subject",
            "retrieve reviewed consumer-health topic records",
        )],
        "OpenAlex" => vec![disclosure(
            "openalex",
            "OpenAlex",
            &["api.openalex.org"],
            "a minimized scholarly query and optional recent-date filter",
            "retrieve scholarly work metadata",
        )],
        "Open Library" => vec![disclosure(
            "open-library",
            "Open Library",
            &["openlibrary.org"],
            "the public book or textbook query",
            "retrieve book metadata and access indicators",
        )],
        "World Bank Open Data" => vec![disclosure(
            "world-bank",
            "World Bank Open Data",
            &["api.worldbank.org"],
            "country names or codes and reviewed indicator identifiers",
            "retrieve comparable public indicators",
        )],
        _ => Vec::new(),
    }
}

pub fn default_client() -> Result<Client> {
    Client::builder()
        .user_agent(format!(
            "BarnLabs-Inquiry/{} (+https://barnlabs.net; public research client)",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("could not create HTTP client")
}

pub struct WikipediaSource {
    client: Client,
}

pub struct GdeltDocSource {
    client: Client,
}

impl GdeltDocSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PublicSource for GdeltDocSource {
    fn name(&self) -> &'static str {
        "GDELT DOC"
    }

    fn supports(&self, plan: &ResearchPlan) -> bool {
        matches!(
            resolve_intent(&plan.query).kind,
            IntentKind::RecentEventMedia
        )
    }

    async fn search(&self, plan: &ResearchPlan, limit: usize) -> Result<SourceOutput> {
        let query = gdelt_event_query(&plan.query)?;
        let mut request_url = Url::parse("https://api.gdeltproject.org/api/v2/doc/doc")?;
        request_url
            .query_pairs_mut()
            .append_pair("query", &query)
            .append_pair("mode", "ArtList")
            .append_pair("format", "json")
            .append_pair("maxrecords", &limit.clamp(1, 10).to_string())
            .append_pair("timespan", "3months")
            .append_pair("sort", "datedesc");
        let response = self.client.get(request_url.clone()).send().await?;
        if response.status().as_u16() == 429 {
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .map(|value| value.chars().take(64).collect::<String>());
            bail!(match retry_after {
                Some(value) =>
                    format!("GDELT DOC rate limit reached; do not retry before Retry-After {value}"),
                None => "GDELT DOC rate limit reached; Inquiry did not retry automatically".into(),
            });
        }
        let response = response.error_for_status()?;
        if response.url() != &request_url {
            bail!("GDELT DOC redirected outside the exact API endpoint");
        }
        let value: Value = json_limited(response, 4_000_000, "GDELT DOC").await?;
        let articles = value
            .get("articles")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("GDELT DOC response omitted its article list"))?;
        let now = Utc::now();
        let mut findings = Vec::new();
        let mut sources = Vec::new();
        for article in articles.iter().take(limit.clamp(1, 10)) {
            let Some(url) = article.get("url").and_then(Value::as_str) else {
                continue;
            };
            if url.len() > 2_048 || safe_http_url(url).is_none() {
                continue;
            }
            let title = article
                .get("title")
                .and_then(Value::as_str)
                .map(|value| truncate_chars(value, 240))
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "Untitled GDELT article lead".into());
            let publisher = article
                .get("domain")
                .and_then(Value::as_str)
                .filter(|value| value.len() <= 253)
                .map(str::to_owned)
                .or_else(|| {
                    Url::parse(url)
                        .ok()
                        .and_then(|value| value.host_str().map(str::to_owned))
                })
                .unwrap_or_else(|| "Publisher indexed by GDELT".into());
            let seen = article
                .get("seendate")
                .and_then(Value::as_str)
                .filter(|value| value.len() <= 32)
                .map(str::to_owned);
            let social_image = article
                .get("socialimage")
                .and_then(Value::as_str)
                .filter(|value| value.len() <= 2_048)
                .filter(|value| {
                    Url::parse(value).is_ok_and(|url| {
                        url.scheme() == "https"
                            && url.username().is_empty()
                            && url.password().is_none()
                            && url.port().is_none()
                    })
                })
                .map(str::to_owned);
            let source_id = stable_id("gdelt-article", url);
            findings.push(Finding {
                id: stable_id("finding", &format!("{source_id}:{title}")),
                title: title.clone(),
                body: "GDELT indexed this as a recent event-related article lead. Inquiry has not treated the headline, publisher image, or official statements as independently verified facts; compare the cited article with primary and independent records.".into(),
                facet: Facet::News,
                confidence: Confidence::Low,
                source_ids: vec![source_id.clone()],
                content_trust: ContentTrust::ExternalUntrusted,
                tags: vec![
                    "event-discovery".into(),
                    "gdelt-index".into(),
                    "rights-unverified-image-candidate".into(),
                ],
            });
            sources.push(SourceRecord {
                id: source_id,
                title,
                url: url.into(),
                publisher,
                retrieved_at: now,
                published_at: seen.clone(),
                license: None,
                source_type: SourceType::News,
                quality: SourceQuality::DiscoveryOnly,
                content_hash: Some(hash_text(&serde_json::to_string(article)?)),
                provenance: ProvenanceDetails {
                    dataset_id: Some("GDELT DOC 2.0 rolling article index".into()),
                    request_url: Some(request_url.to_string()),
                    methodology_url: Some(
                        "https://blog.gdeltproject.org/gdelt-doc-2-0-api-debuts/".into(),
                    ),
                    observation_period: Some("rolling three months".into()),
                    source_updated_at: seen,
                    content_url: social_image,
                    media_role: Some("unverified_event_image_candidate".into()),
                    ..Default::default()
                },
            });
        }
        if findings.is_empty() {
            bail!("GDELT DOC returned no usable recent event leads");
        }
        Ok(SourceOutput {
            connector: self.name().into(),
            findings,
            metrics: Vec::new(),
            sources,
            warnings: vec![
                "GDELT supplies news and social-image discovery metadata, not event verification or image reuse permission. Inquiry never embeds a GDELT social-image URL without a separate file-specific rights record.".into(),
            ],
            audit: ConnectorAudit {
                attempted: vec!["GDELT DOC 2.0 API".into()],
                succeeded: vec!["GDELT DOC 2.0 API".into()],
                errors: Vec::new(),
            },
        })
    }
}

fn gdelt_event_query(query: &str) -> Result<String> {
    let ignored = [
        "show", "find", "give", "display", "me", "a", "an", "the", "of", "image", "images",
        "photo", "photos", "picture", "pictures", "recent", "latest", "current", "please",
    ];
    let value = query
        .to_ascii_lowercase()
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() {
                value
            } else {
                ' '
            }
        })
        .collect::<String>();
    let terms = value
        .split_whitespace()
        .filter(|term| !ignored.contains(term))
        .take(8)
        .collect::<Vec<_>>();
    if terms.is_empty() {
        bail!("recent-event query did not retain a searchable public event subject");
    }
    Ok(terms.join(" "))
}

pub struct WikidataOfficeholderSource {
    client: Client,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UkParliamentMonarchRecord {
    name: String,
    reign_started: String,
    birth_date: String,
    wikidata_id: String,
}

impl WikidataOfficeholderSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PublicSource for WikidataOfficeholderSource {
    fn name(&self) -> &'static str {
        "Wikidata current officeholder"
    }

    fn supports(&self, plan: &ResearchPlan) -> bool {
        current_office_search_phrase(&plan.query).is_some()
    }

    async fn search(&self, plan: &ResearchPlan, _: usize) -> Result<SourceOutput> {
        let office_phrase = current_office_search_phrase(&plan.query)
            .ok_or_else(|| anyhow!("query did not identify a supported current office"))?;
        let mut search_url = Url::parse("https://www.wikidata.org/w/api.php")?;
        search_url
            .query_pairs_mut()
            .append_pair("action", "wbsearchentities")
            .append_pair("format", "json")
            .append_pair("language", "en")
            .append_pair("uselang", "en")
            .append_pair("type", "item")
            .append_pair("limit", "5")
            .append_pair("search", &office_phrase);
        let search_response = self
            .client
            .get(search_url.clone())
            .send()
            .await?
            .error_for_status()?;
        let search: Value =
            json_limited(search_response, 1_000_000, "Wikidata entity search").await?;
        let office = search
            .get("search")
            .and_then(Value::as_array)
            .and_then(|results| {
                results
                    .iter()
                    .find(|result| wikidata_office_result_matches(result, &office_phrase))
            })
            .ok_or_else(|| {
                anyhow!(
                    "Wikidata did not return an exact government-office match for '{office_phrase}'"
                )
            })?;
        let office_id = office
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Wikidata office match omitted its entity ID"))?;

        if let Some(ordinal) = query_office_ordinal(&plan.query) {
            return self
                .ordinal_officeholder(office_id, &office_phrase, &ordinal)
                .await;
        }

        let (office_url, office_entity) = self.entity(office_id).await?;
        let statements = office_entity
            .pointer("/claims/P1308")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("matched office has no current-officeholder statement"))?;
        let statement = select_current_officeholder_statement(statements)?.ok_or_else(|| {
            anyhow!("matched office has no unexpired current-officeholder statement")
        })?;
        let person_id = statement
            .pointer("/mainsnak/datavalue/value/id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("current-officeholder statement omitted the person ID"))?;
        let (person_request_url, person) = self.entity(person_id).await?;
        let person_name = person
            .pointer("/labels/en/value")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("officeholder entity omitted its English label"))?;
        let description = person
            .pointer("/descriptions/en/value")
            .and_then(Value::as_str)
            .unwrap_or("No English description was supplied.");
        let is_us_office = office_labels_match(&office_phrase, "President of the United States");
        let is_uk_monarch = office_labels_match(&office_phrase, "Monarch of the United Kingdom");
        let display_name = officeholder_display_name(&office_phrase, person_name, &person);
        let mut audit = ConnectorAudit {
            attempted: vec!["Wikidata API".into()],
            succeeded: vec!["Wikidata API".into()],
            errors: Vec::new(),
        };
        let wikipedia_summary = if let Some(title) = person
            .pointer("/sitelinks/enwiki/title")
            .and_then(Value::as_str)
        {
            audit.attempted.push("Wikipedia REST summary".into());
            match self.wikipedia_summary(title).await {
                Ok(summary) => {
                    audit.succeeded.push("Wikipedia REST summary".into());
                    summary
                }
                Err(error) => {
                    audit.errors.push(format!(
                        "Wikipedia REST summary: {}",
                        compact_source_error(&error)
                    ));
                    None
                }
            }
        } else {
            None
        };
        let mut warnings = Vec::new();
        audit.attempted.push("Wikimedia Commons API".into());
        let portrait_source = match self
            .commons_portrait_source(person_id, &display_name, &person)
            .await
        {
            Ok(source) => {
                if source.is_some() {
                    audit.succeeded.push("Wikimedia Commons API".into());
                } else {
                    audit.errors.push(
                        "Wikimedia Commons API: resolved entity has no accepted P18 portrait"
                            .into(),
                    );
                }
                source
            }
            Err(error) => {
                warnings.push(format!(
                    "The identity-bound Wikidata P18 portrait was not accepted: {error}"
                ));
                audit.errors.push(format!(
                    "Wikimedia Commons API: {}",
                    compact_source_error(&error)
                ));
                None
            }
        };
        let (official_sources, official_warnings, official_audit, official_biography) =
            if is_us_office {
                let (sources, warnings, audit) = self.official_us_sources(person_name).await;
                (sources, warnings, audit, None)
            } else if is_uk_monarch {
                self.official_uk_sources(person_id, person_name, &display_name)
                    .await
            } else {
                (Vec::new(), Vec::new(), ConnectorAudit::default(), None)
            };
        warnings.extend(official_warnings);
        audit.attempted.extend(official_audit.attempted);
        audit.succeeded.extend(official_audit.succeeded);
        audit.errors.extend(official_audit.errors);
        let start = qualifier_time(statement, "P580");
        let ordinal = statement
            .pointer("/qualifiers/P1545/0/datavalue/value")
            .and_then(Value::as_str);
        let wikidata_url = format!("https://www.wikidata.org/wiki/{person_id}");
        let source_id = stable_id("wikidata-officeholder", &wikidata_url);
        let position = ordinal
            .map(|value| format!("the {} officeholder", ordinal_label(value)))
            .unwrap_or_else(|| "the current officeholder".into());
        let start_text = start
            .as_deref()
            .map(|value| format!(" The structured term-start qualifier is {value}."))
            .unwrap_or_default();
        let biography = official_biography.unwrap_or_else(|| {
            wikipedia_summary
                .as_ref()
                .and_then(|(_, summary)| summary.get("extract").and_then(Value::as_str))
                .map(|extract| truncate_chars(extract, 1_200))
                .unwrap_or_else(|| description.to_owned())
        });
        let has_uk_parliament = official_sources
            .iter()
            .any(|source| source.publisher == "UK Parliament");
        let has_royal_family = official_sources
            .iter()
            .any(|source| source.publisher == "The Royal Family");
        let official_note = if official_sources
            .iter()
            .any(|source| source.publisher == "USAGov")
        {
            " USAGov independently corroborated the current-office identity in this run."
        } else if has_uk_parliament && has_royal_family {
            " UK Parliament's current-reign data and The Royal Family's official profile independently corroborated the current-office identity in this run."
        } else if has_uk_parliament {
            " UK Parliament's current-reign data independently corroborated the current-office identity in this run."
        } else if has_royal_family {
            " The Royal Family's official profile corroborated the current-office identity in this run."
        } else {
            " No independent official current-office record was accepted in this run."
        };
        let source_caveat = if is_us_office {
            "The Wikipedia summary is community-maintained secondary evidence. Any linked White House biography is administration-controlled and may contain promotional claims; compare primary and archival records before consequential use."
        } else if is_uk_monarch {
            "The Royal Family profile is institution-controlled; compare UK Parliament, archival, and independent records for contested biographical claims."
        } else {
            "The Wikipedia summary is community-maintained secondary evidence; compare relevant government and archival records before consequential use."
        };
        let body = format!(
            "Wikidata currently identifies {display_name} as {position} for {office_phrase}.{start_text}{official_note} Biographical summary: {biography} {source_caveat}"
        );
        let now = Utc::now();
        let mut finding_source_ids = vec![source_id.clone()];
        let mut source_records = vec![SourceRecord {
            id: source_id.clone(),
            title: format!("Wikidata officeholder statement: {person_name}"),
            url: wikidata_url,
            publisher: "Wikimedia Foundation / Wikidata community".into(),
            retrieved_at: now,
            published_at: start.clone(),
            license: Some("Wikidata structured data: CC0".into()),
            source_type: SourceType::Encyclopedia,
            quality: SourceQuality::StrongSecondary,
            content_hash: Some(hash_text(&format!(
                "{}{}",
                serde_json::to_string(statement)?,
                serde_json::to_string(&person)?
            ))),
            provenance: ProvenanceDetails {
                dataset_id: Some(format!("Wikidata {office_id} P1308 -> {person_id}")),
                request_url: Some(format!("{office_url}; {person_request_url}")),
                methodology_url: Some("https://www.wikidata.org/wiki/Property:P1308".into()),
                observation_period: start,
                ..Default::default()
            },
        }];
        if let Some((request_url, summary)) = wikipedia_summary {
            let page_url = summary
                .pointer("/content_urls/desktop/page")
                .and_then(Value::as_str)
                .and_then(safe_http_url)
                .unwrap_or("https://en.wikipedia.org/")
                .to_owned();
            let wikipedia_id = stable_id("wikipedia-summary", &page_url);
            finding_source_ids.push(wikipedia_id.clone());
            source_records.push(SourceRecord {
                id: wikipedia_id,
                title: format!("Wikipedia biography summary: {person_name}"),
                url: page_url,
                publisher: "Wikimedia Foundation / Wikipedia contributors".into(),
                retrieved_at: now,
                published_at: None,
                license: Some("CC BY-SA 4.0; page-specific attribution applies".into()),
                source_type: SourceType::Encyclopedia,
                quality: SourceQuality::StrongSecondary,
                content_hash: Some(hash_text(&serde_json::to_string(&summary)?)),
                provenance: ProvenanceDetails {
                    dataset_id: Some(format!("English Wikipedia summary for {person_id}")),
                    request_url: Some(request_url),
                    ..Default::default()
                },
            });
        }
        if let Some(source) = portrait_source {
            finding_source_ids.push(source.id.clone());
            source_records.push(source);
        }
        for source in official_sources {
            finding_source_ids.push(source.id.clone());
            source_records.push(source);
        }
        let finding_title = if is_uk_monarch {
            format!("Current UK monarch: {display_name}")
        } else {
            format!("Current officeholder: {display_name}")
        };
        Ok(SourceOutput {
            connector: self.name().into(),
            findings: vec![Finding {
                id: stable_id("finding", &format!("{source_id}:{display_name}")),
                title: finding_title,
                body: body.clone(),
                facet: Facet::Overview,
                confidence: Confidence::Moderate,
                source_ids: finding_source_ids,
                content_trust: ContentTrust::ExternalUntrusted,
                tags: vec![
                    "current-officeholder".into(),
                    "exact-office-match".into(),
                    "structured-data".into(),
                ],
            }],
            metrics: Vec::new(),
            sources: source_records,
            warnings,
            audit,
        })
    }
}

impl WikidataOfficeholderSource {
    async fn ordinal_officeholder(
        &self,
        office_id: &str,
        office_phrase: &str,
        ordinal: &str,
    ) -> Result<SourceOutput> {
        if !office_id.starts_with('Q')
            || !office_id[1..]
                .chars()
                .all(|character| character.is_ascii_digit())
            || !ordinal.chars().all(|character| character.is_ascii_digit())
        {
            bail!("Wikidata returned an invalid office or ordinal identifier");
        }
        let sparql = format!(
            "SELECT ?person ?personLabel ?start ?end WHERE {{ ?person p:P39 ?statement. ?statement ps:P39 wd:{office_id}; pq:P1545 \"{ordinal}\". OPTIONAL {{ ?statement pq:P580 ?start. }} OPTIONAL {{ ?statement pq:P582 ?end. }} SERVICE wikibase:label {{ bd:serviceParam wikibase:language \"en\". }} }} LIMIT 5"
        );
        let mut request_url = Url::parse("https://query.wikidata.org/sparql")?;
        request_url
            .query_pairs_mut()
            .append_pair("format", "json")
            .append_pair("query", &sparql);
        let response = self
            .client
            .get(request_url.clone())
            .send()
            .await?
            .error_for_status()?;
        let root: Value = json_limited(response, 2_000_000, "Wikidata Query Service").await?;
        let bindings = root
            .pointer("/results/bindings")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("Wikidata ordinal query omitted result bindings"))?;
        let mut people = BTreeMap::new();
        for binding in bindings {
            let Some(uri) = binding.pointer("/person/value").and_then(Value::as_str) else {
                continue;
            };
            let Some(person_id) = uri.rsplit('/').next().filter(|id| id.starts_with('Q')) else {
                continue;
            };
            people.entry(person_id.to_owned()).or_insert(binding);
        }
        if people.len() != 1 {
            bail!(
                "Wikidata returned {} distinct people for the {} {} query; Inquiry abstained instead of choosing",
                people.len(),
                ordinal_label(ordinal),
                office_phrase
            );
        }
        let (person_id, binding) = people.into_iter().next().expect("one person checked");
        let (person_request_url, person) = self.entity(&person_id).await?;
        let mut audit = ConnectorAudit {
            attempted: vec!["Wikidata API".into(), "Wikidata Query Service".into()],
            succeeded: vec!["Wikidata API".into(), "Wikidata Query Service".into()],
            errors: Vec::new(),
        };
        let person_name = person
            .pointer("/labels/en/value")
            .and_then(Value::as_str)
            .or_else(|| {
                binding
                    .pointer("/personLabel/value")
                    .and_then(Value::as_str)
            })
            .ok_or_else(|| anyhow!("ordinal officeholder result omitted the person's name"))?;
        let description = person
            .pointer("/descriptions/en/value")
            .and_then(Value::as_str)
            .unwrap_or("No English description was supplied.");
        let wikipedia_summary = if let Some(title) = person
            .pointer("/sitelinks/enwiki/title")
            .and_then(Value::as_str)
        {
            audit.attempted.push("Wikipedia REST summary".into());
            match self.wikipedia_summary(title).await {
                Ok(summary) => {
                    audit.succeeded.push("Wikipedia REST summary".into());
                    summary
                }
                Err(error) => {
                    audit.errors.push(format!(
                        "Wikipedia REST summary: {}",
                        compact_source_error(&error)
                    ));
                    None
                }
            }
        } else {
            None
        };
        let biography = wikipedia_summary
            .as_ref()
            .and_then(|(_, summary)| summary.get("extract").and_then(Value::as_str))
            .map(|extract| truncate_chars(extract, 1_200))
            .unwrap_or_else(|| description.to_owned());
        let start = binding
            .pointer("/start/value")
            .and_then(Value::as_str)
            .and_then(|value| value.get(..10))
            .map(str::to_owned);
        let end = binding
            .pointer("/end/value")
            .and_then(Value::as_str)
            .and_then(|value| value.get(..10))
            .map(str::to_owned);
        let term = match (&start, &end) {
            (Some(start), Some(end)) => format!(" The structured term is {start} through {end}."),
            (Some(start), None) => format!(" The structured term starts {start}."),
            _ => String::new(),
        };
        let wikidata_url = format!("https://www.wikidata.org/wiki/{person_id}");
        let source_id = stable_id(
            "wikidata-ordinal-officeholder",
            &format!("{office_id}:{ordinal}:{person_id}"),
        );
        let now = Utc::now();
        let mut source_ids = vec![source_id.clone()];
        let mut sources = vec![SourceRecord {
            id: source_id.clone(),
            title: format!(
                "Wikidata position statement: {} {office_phrase}",
                ordinal_label(ordinal)
            ),
            url: wikidata_url,
            publisher: "Wikimedia Foundation / Wikidata community".into(),
            retrieved_at: now,
            published_at: start.clone(),
            license: Some("Wikidata structured data: CC0".into()),
            source_type: SourceType::Encyclopedia,
            quality: SourceQuality::StrongSecondary,
            content_hash: Some(hash_text(&serde_json::to_string(binding)?)),
            provenance: ProvenanceDetails {
                dataset_id: Some(format!("Wikidata P39 {office_id}, ordinal {ordinal}")),
                request_url: Some(format!("{}; {person_request_url}", request_url)),
                methodology_url: Some("https://www.wikidata.org/wiki/Property:P1545".into()),
                observation_period: match (&start, &end) {
                    (Some(start), Some(end)) => Some(format!("{start} to {end}")),
                    (Some(start), None) => Some(format!("from {start}")),
                    _ => None,
                },
                ..Default::default()
            },
        }];
        if let Some((summary_request_url, summary)) = wikipedia_summary {
            let page_url = summary
                .pointer("/content_urls/desktop/page")
                .and_then(Value::as_str)
                .and_then(safe_http_url)
                .unwrap_or("https://en.wikipedia.org/")
                .to_owned();
            let wikipedia_id = stable_id("wikipedia-summary", &page_url);
            source_ids.push(wikipedia_id.clone());
            sources.push(SourceRecord {
                id: wikipedia_id,
                title: format!("Wikipedia biography summary: {person_name}"),
                url: page_url,
                publisher: "Wikimedia Foundation / Wikipedia contributors".into(),
                retrieved_at: now,
                published_at: None,
                license: Some("CC BY-SA 4.0; page-specific attribution applies".into()),
                source_type: SourceType::Encyclopedia,
                quality: SourceQuality::StrongSecondary,
                content_hash: Some(hash_text(&serde_json::to_string(&summary)?)),
                provenance: ProvenanceDetails {
                    dataset_id: Some(format!("English Wikipedia summary for {person_id}")),
                    request_url: Some(summary_request_url),
                    ..Default::default()
                },
            });
        }
        Ok(SourceOutput {
            connector: self.name().into(),
            findings: vec![Finding {
                id: stable_id("finding", &format!("{source_id}:{person_name}")),
                title: format!(
                    "{} {}: {person_name}",
                    ordinal_label(ordinal),
                    office_phrase
                ),
                body: format!(
                    "Wikidata identifies {person_name} as the {} officeholder for {office_phrase}.{term} Biographical summary: {biography} Verify the ordinal and term against official constitutional or archival records before consequential use.",
                    ordinal_label(ordinal)
                ),
                facet: Facet::Overview,
                confidence: Confidence::Moderate,
                source_ids,
                content_trust: ContentTrust::ExternalUntrusted,
                tags: vec![
                    "ordinal-officeholder".into(),
                    "exact-ordinal-office-match".into(),
                    "structured-data".into(),
                ],
            }],
            metrics: Vec::new(),
            sources,
            warnings: Vec::new(),
            audit,
        })
    }

    async fn entity(&self, id: &str) -> Result<(String, Value)> {
        let mut request_url = Url::parse("https://www.wikidata.org/w/api.php")?;
        request_url
            .query_pairs_mut()
            .append_pair("action", "wbgetentities")
            .append_pair("format", "json")
            .append_pair("ids", id)
            .append_pair("props", "claims|labels|descriptions|sitelinks")
            .append_pair("languages", "en")
            .append_pair("sitefilter", "enwiki");
        let response = self
            .client
            .get(request_url.clone())
            .send()
            .await?
            .error_for_status()?;
        let root: Value = json_limited(response, 2_000_000, "Wikidata entity").await?;
        let entity = root
            .pointer(&format!("/entities/{id}"))
            .cloned()
            .ok_or_else(|| anyhow!("Wikidata response omitted entity {id}"))?;
        Ok((request_url.to_string(), entity))
    }

    async fn wikipedia_summary(&self, title: &str) -> Result<Option<(String, Value)>> {
        let mut request_url = Url::parse("https://en.wikipedia.org/api/rest_v1/page/summary")?;
        request_url
            .path_segments_mut()
            .map_err(|_| anyhow!("invalid Wikipedia summary URL"))?
            .push(title);
        let response = self
            .client
            .get(request_url.clone())
            .send()
            .await?
            .error_for_status()?;
        let summary: Value = json_limited(response, 2_000_000, "Wikipedia summary").await?;
        Ok(Some((request_url.to_string(), summary)))
    }

    async fn commons_portrait_source(
        &self,
        person_id: &str,
        person_name: &str,
        person: &Value,
    ) -> Result<Option<SourceRecord>> {
        let Some(statements) = person.pointer("/claims/P18").and_then(Value::as_array) else {
            return Ok(None);
        };
        let statement = statements
            .iter()
            .filter(|statement| statement.get("rank").and_then(Value::as_str) != Some("deprecated"))
            .find(|statement| statement.get("rank").and_then(Value::as_str) == Some("preferred"))
            .or_else(|| {
                statements.iter().find(|statement| {
                    statement.get("rank").and_then(Value::as_str) != Some("deprecated")
                })
            });
        let Some(filename) = statement
            .and_then(|statement| statement.pointer("/mainsnak/datavalue/value"))
            .and_then(Value::as_str)
            .filter(|filename| !filename.is_empty() && filename.chars().count() <= 500)
        else {
            return Ok(None);
        };

        let mut request_url = Url::parse("https://commons.wikimedia.org/w/api.php")?;
        request_url
            .query_pairs_mut()
            .append_pair("action", "query")
            .append_pair("format", "json")
            .append_pair("formatversion", "2")
            .append_pair("prop", "imageinfo")
            .append_pair("titles", &format!("File:{filename}"))
            .append_pair("iiprop", "url|mime|size|sha1|extmetadata")
            .append_pair(
                "iiextmetadatafilter",
                "LicenseShortName|LicenseUrl|Artist|ImageDescription|Credit",
            )
            .append_pair("iiurlwidth", "640");
        let response = self
            .client
            .get(request_url.clone())
            .send()
            .await?
            .error_for_status()?;
        let root: Value = json_limited(response, 4_000_000, "exact Commons portrait").await?;
        let Some(page) = root.pointer("/query/pages/0") else {
            return Ok(None);
        };
        let returned_title = page
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim_start_matches("File:")
            .replace('_', " ");
        if normalize_for_matching(&returned_title) != normalize_for_matching(filename) {
            bail!("Commons returned a different filename for the exact P18 portrait lookup");
        }
        let info = page
            .pointer("/imageinfo/0")
            .ok_or_else(|| anyhow!("Commons returned no image metadata for the P18 portrait"))?;
        let mime = info
            .get("mime")
            .and_then(Value::as_str)
            .filter(|value| matches!(*value, "image/jpeg" | "image/png" | "image/webp"))
            .ok_or_else(|| anyhow!("Commons P18 record was not a supported raster image"))?;
        let size = info
            .get("size")
            .and_then(Value::as_u64)
            .filter(|value| *value > 0 && *value <= 20_000_000)
            .ok_or_else(|| anyhow!("Commons P18 image omitted a safe bounded byte size"))?;
        let width = info
            .get("width")
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
            .ok_or_else(|| anyhow!("Commons P18 image omitted its width"))?;
        let height = info
            .get("height")
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
            .ok_or_else(|| anyhow!("Commons P18 image omitted its height"))?;
        if width.saturating_mul(height) > 40_000_000 {
            bail!("Commons P18 image exceeds Inquiry's pixel-count limit");
        }
        let description_url = strict_media_https_url(
            info.get("descriptionurl").and_then(Value::as_str),
            &["commons.wikimedia.org"],
        )?;
        let content_url = strict_media_https_url(
            info.get("url").and_then(Value::as_str),
            &["upload.wikimedia.org"],
        )?;
        let preview_url = strict_media_https_url(
            info.get("thumburl").and_then(Value::as_str),
            &["upload.wikimedia.org"],
        )?;
        let description = commons_metadata(info, "ImageDescription")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("Commons P18 image omitted a description"))?;
        if !portrait_metadata_identifies_person(filename, &description, person_name) {
            bail!("Commons P18 filename and description did not identify the resolved person");
        }
        let creator = commons_metadata(info, "Artist")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("Commons P18 image omitted creator metadata"))?;
        let credit = commons_metadata(info, "Credit")
            .filter(|value| !value.is_empty())
            .map(|value| {
                if contains_phrase(&value, "White House") {
                    "White House".into()
                } else {
                    value
                }
            });
        let license_name = commons_metadata(info, "LicenseShortName")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("Commons P18 image omitted reuse terms"))?;
        let license_url = commons_metadata(info, "LicenseUrl")
            .filter(|value| Url::parse(value).is_ok_and(|url| url.scheme() == "https"));
        if !recognized_commons_license(&license_name, license_url.as_deref()) {
            bail!(
                "Commons P18 image did not identify a recognized public-domain or Creative Commons reuse basis"
            );
        }
        let license = license_url
            .as_ref()
            .map(|url| format!("{license_name}; {url}"))
            .unwrap_or_else(|| license_name.clone());
        let source_id = stable_id("commons-p18-portrait", &description_url);
        Ok(Some(SourceRecord {
            id: source_id,
            title: format!("Identity portrait of {person_name}: {filename}"),
            url: description_url,
            publisher: "Wikimedia Commons contributor community".into(),
            retrieved_at: Utc::now(),
            published_at: None,
            license: Some(license),
            source_type: SourceType::Other,
            quality: SourceQuality::StrongSecondary,
            content_hash: info.get("sha1").and_then(Value::as_str).map(str::to_owned),
            provenance: ProvenanceDetails {
                dataset_id: Some(format!("Wikidata {person_id} P18 -> {filename}")),
                request_url: Some(request_url.to_string()),
                methodology_url: Some("https://www.wikidata.org/wiki/Property:P18".into()),
                content_url: Some(content_url),
                preview_url: Some(preview_url),
                file_format: Some(mime.into()),
                file_size_bytes: Some(size),
                width_pixels: Some(width),
                height_pixels: Some(height),
                creator: Some(creator),
                credit,
                license_url,
                alt_text: Some(format!("Portrait of {person_name}")),
                media_role: Some("identity_portrait".into()),
                subject_entity_id: Some(person_id.into()),
                ..Default::default()
            },
        }))
    }

    async fn official_us_sources(
        &self,
        person_name: &str,
    ) -> (Vec<SourceRecord>, Vec<String>, ConnectorAudit) {
        let mut sources = Vec::new();
        let mut warnings = Vec::new();
        let mut audit = ConnectorAudit::default();
        let usa_url = "https://www.usa.gov/presidents";
        audit.attempted.push("USAGov".into());
        let usa_outcome = async {
            let response = self.client.get(usa_url).send().await?.error_for_status()?;
            let bytes = bytes_limited(response, 3_000_000, "USAGov presidents").await?;
            let html = std::str::from_utf8(&bytes).context("USAGov page was not UTF-8")?;
            let section = usagov_current_president_section(html)
                .ok_or_else(|| anyhow!("USAGov current-president section was not found"))?;
            let section_text = normalize_for_matching(&strip_markup(section));
            if !contains_phrase(&section_text, "current president of the united states")
                || !person_name_tokens_match(&section_text, person_name)
            {
                bail!("USAGov current-president section did not match the resolved full name");
            }
            let white_house_url =
                Regex::new(r#"https://www\.whitehouse\.gov/administration/[A-Za-z0-9_-]+/"#)
                    .expect("valid White House profile-link regex")
                    .find(section)
                    .map(|matched| matched.as_str().to_owned());
            Ok::<_, anyhow::Error>((
                SourceRecord {
                    id: stable_id("official-us-officeholder", usa_url),
                    title: format!("USAGov current-president record: {person_name}"),
                    url: usa_url.into(),
                    publisher: "USAGov".into(),
                    retrieved_at: Utc::now(),
                    published_at: None,
                    license: None,
                    source_type: SourceType::Government,
                    quality: SourceQuality::Primary,
                    content_hash: Some(hash_text(section)),
                    provenance: ProvenanceDetails {
                        dataset_id: Some("USAGov current-president section".into()),
                        request_url: Some(usa_url.into()),
                        methodology_url: Some("https://www.usa.gov/about-the-us".into()),
                        ..Default::default()
                    },
                },
                white_house_url,
            ))
        }
        .await;
        let white_house_url = match usa_outcome {
            Ok((source, profile_url)) => {
                audit.succeeded.push("USAGov".into());
                sources.push(source);
                profile_url
            }
            Err(error) => {
                audit
                    .errors
                    .push(format!("USAGov: {}", compact_source_error(&error)));
                warnings.push(format!(
                    "USAGov current-president record was not accepted: {error}"
                ));
                None
            }
        };

        if let Some(url) = white_house_url {
            audit.attempted.push("White House".into());
            let outcome = async {
                let response = self.client.get(&url).send().await?.error_for_status()?;
                let bytes = bytes_limited(response, 3_000_000, "White House biography").await?;
                let html =
                    std::str::from_utf8(&bytes).context("White House biography was not UTF-8")?;
                let title = white_house_identity_title(html).ok_or_else(|| {
                    anyhow!("White House biography omitted an identity-bearing title")
                })?;
                let normalized_title = normalize_for_matching(&title);
                if !contains_phrase(&normalized_title, "president")
                    || !person_name_tokens_match(&normalized_title, person_name)
                {
                    bail!("White House biography title did not match the resolved full name");
                }
                Ok::<_, anyhow::Error>(SourceRecord {
                    id: stable_id("official-us-biography", &url),
                    title: format!("White House administration biography: {person_name}"),
                    url: url.clone(),
                    publisher: "The White House".into(),
                    retrieved_at: Utc::now(),
                    published_at: None,
                    license: None,
                    source_type: SourceType::Government,
                    quality: SourceQuality::Primary,
                    content_hash: Some(hash_text(html)),
                    provenance: ProvenanceDetails {
                        dataset_id: Some("Administration-controlled biography".into()),
                        request_url: Some(url),
                        ..Default::default()
                    },
                })
            }
            .await;
            match outcome {
                Ok(source) => {
                    audit.succeeded.push("White House".into());
                    sources.push(source)
                }
                Err(error) => {
                    audit
                        .errors
                        .push(format!("White House: {}", compact_source_error(&error)));
                    warnings.push(format!(
                        "White House administration biography was not accepted: {error}"
                    ))
                }
            }
        } else {
            warnings.push(
                "USAGov did not expose a White House biography link for the resolved person."
                    .into(),
            );
        }
        (sources, warnings, audit)
    }

    async fn official_uk_sources(
        &self,
        person_id: &str,
        person_name: &str,
        display_name: &str,
    ) -> (
        Vec<SourceRecord>,
        Vec<String>,
        ConnectorAudit,
        Option<String>,
    ) {
        let mut sources = Vec::new();
        let mut warnings = Vec::new();
        let mut audit = ConnectorAudit::default();
        let reigns_url = "https://api.parliament.uk/regnal-years/reigns.csv";
        let monarchs_url = "https://api.parliament.uk/regnal-years/monarchs.csv";

        audit.attempted.push("UK Parliament".into());
        let parliament_outcome = async {
            let reigns_response = self
                .client
                .get(reigns_url)
                .send()
                .await?
                .error_for_status()?;
            let reigns_bytes =
                bytes_limited(reigns_response, 500_000, "UK Parliament reigns CSV").await?;
            let monarchs_response = self
                .client
                .get(monarchs_url)
                .send()
                .await?
                .error_for_status()?;
            let monarchs_bytes =
                bytes_limited(monarchs_response, 500_000, "UK Parliament monarchs CSV").await?;
            let reigns_csv =
                std::str::from_utf8(&reigns_bytes).context("UK Parliament reigns CSV was not UTF-8")?;
            let monarchs_csv = std::str::from_utf8(&monarchs_bytes)
                .context("UK Parliament monarchs CSV was not UTF-8")?;
            let record = current_uk_monarch_from_parliament_csv(reigns_csv, monarchs_csv)?;
            if normalize_for_matching(&record.name) != normalize_for_matching(person_name)
                || record.wikidata_id != person_id
            {
                bail!(
                    "UK Parliament current-monarch data did not match the resolved Wikidata identity"
                );
            }
            let source = SourceRecord {
                id: stable_id("official-uk-monarch", reigns_url),
                title: format!("UK Parliament current-reign record: {display_name}"),
                url: "https://api.parliament.uk/regnal-years/reigns".into(),
                publisher: "UK Parliament".into(),
                retrieved_at: Utc::now(),
                published_at: Some(record.reign_started.clone()),
                license: None,
                source_type: SourceType::Government,
                quality: SourceQuality::Primary,
                content_hash: Some(hash_text(&format!("{reigns_csv}\n{monarchs_csv}"))),
                provenance: ProvenanceDetails {
                    dataset_id: Some(
                        "UK Parliament Session Citations current reign and monarch tables".into(),
                    ),
                    request_url: Some(format!("{reigns_url}; {monarchs_url}")),
                    methodology_url: Some("https://api.parliament.uk/regnal-years".into()),
                    observation_period: Some(format!("{}–present", record.reign_started)),
                    ..Default::default()
                },
            };
            Ok::<_, anyhow::Error>((source, record))
        }
        .await;
        let parliament_record = match parliament_outcome {
            Ok((source, record)) => {
                audit.succeeded.push("UK Parliament".into());
                sources.push(source);
                Some(record)
            }
            Err(error) => {
                audit
                    .errors
                    .push(format!("UK Parliament: {}", compact_source_error(&error)));
                warnings.push(format!(
                    "UK Parliament current-monarch data was not accepted: {error}"
                ));
                None
            }
        };

        let royal_url = "https://www.royal.uk/the-king";
        audit.attempted.push("The Royal Family".into());
        let royal_outcome = async {
            let response = self
                .client
                .get(royal_url)
                .send()
                .await?
                .error_for_status()?;
            let bytes = bytes_limited(response, 3_000_000, "Royal Family biography").await?;
            let html =
                std::str::from_utf8(&bytes).context("Royal Family biography was not UTF-8")?;
            royal_family_identity_summary(html, person_name).ok_or_else(|| {
                anyhow!("Royal Family biography did not match the resolved monarch")
            })?;
            Ok::<_, anyhow::Error>(SourceRecord {
                id: stable_id("official-royal-biography", royal_url),
                title: format!("Royal Family official biography: {display_name}"),
                url: royal_url.into(),
                publisher: "The Royal Family".into(),
                retrieved_at: Utc::now(),
                published_at: None,
                license: None,
                source_type: SourceType::Other,
                quality: SourceQuality::Primary,
                content_hash: Some(hash_text(html)),
                provenance: ProvenanceDetails {
                    dataset_id: Some("Royal Household official biography page".into()),
                    request_url: Some(royal_url.into()),
                    ..Default::default()
                },
            })
        }
        .await;
        let royal_accepted = match royal_outcome {
            Ok(source) => {
                audit.succeeded.push("The Royal Family".into());
                sources.push(source);
                true
            }
            Err(error) => {
                audit.errors.push(format!(
                    "The Royal Family: {}",
                    compact_source_error(&error)
                ));
                warnings.push(format!(
                    "The Royal Family official biography was not accepted: {error}"
                ));
                false
            }
        };

        let biography = parliament_record.map(|record| {
            let birth = human_readable_iso_date(&record.birth_date)
                .unwrap_or_else(|| record.birth_date.clone());
            let reign_started = human_readable_iso_date(&record.reign_started)
                .unwrap_or_else(|| record.reign_started.clone());
            let profile_note = if royal_accepted {
                " The Royal Family's official profile documents the monarch's earlier life and work before accession."
            } else {
                ""
            };
            format!(
                "{display_name} (born {birth}) has reigned as the United Kingdom's monarch since {reign_started}.{profile_note}"
            )
        });
        (sources, warnings, audit, biography)
    }
}
impl WikipediaSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PublicSource for WikipediaSource {
    fn name(&self) -> &'static str {
        "Wikipedia"
    }
    fn supports(&self, plan: &ResearchPlan) -> bool {
        if let Some(office) = current_office_search_phrase(&plan.query) {
            return !office_labels_match(&office, "Monarch of the United Kingdom");
        }
        if matches!(
            resolve_intent(&plan.query).kind,
            IntentKind::RecentEventMedia
        ) {
            return true;
        }
        !office_query_needs_jurisdiction(&plan.query)
            && !is_image_request(&plan.query)
            && !is_scholarly_request(&plan.query)
    }

    async fn search(&self, plan: &ResearchPlan, limit: usize) -> Result<SourceOutput> {
        let mut request_url = Url::parse("https://en.wikipedia.org/w/api.php")?;
        request_url
            .query_pairs_mut()
            .append_pair("action", "query")
            .append_pair("list", "search")
            .append_pair("format", "json")
            .append_pair("utf8", "1")
            .append_pair("srsearch", &wikipedia_search_query(&plan.query))
            .append_pair("srlimit", &limit.min(5).to_string());
        let response = self
            .client
            .get(request_url.clone())
            .send()
            .await?
            .error_for_status()?;
        let response: Value = json_limited(response, 2_000_000, "Wikipedia").await?;
        let items = response
            .pointer("/query/search")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("Wikipedia response did not include search results"))?;
        let strip = Regex::new(r"<[^>]+>").expect("valid regex");
        let now = Utc::now();
        let mut findings = Vec::new();
        let mut sources = Vec::new();
        for item in items {
            let title = item
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Untitled");
            let page_id = item
                .get("pageid")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            let snippet = strip
                .replace_all(
                    item.get("snippet").and_then(Value::as_str).unwrap_or(""),
                    "",
                )
                .to_string();
            if let Some(office_phrase) = current_office_search_phrase(&plan.query)
                && !office_labels_match(title, &office_phrase)
            {
                continue;
            }
            if !query_record_relevant(
                &wikipedia_search_query(&plan.query),
                &format!("{title} {snippet}"),
            ) {
                continue;
            }
            let url = format!("https://en.wikipedia.org/?curid={page_id}");
            let id = stable_id("wikipedia", &url);
            let mut tags = vec!["overview".into()];
            if matches!(
                resolve_intent(&plan.query).kind,
                IntentKind::RecentEventMedia
            ) {
                tags.push("recent-event-candidate".into());
                tags.push(format!(
                    "event-match:{}",
                    event_match_score(&plan.query, &format!("{title} {snippet}"))
                ));
            }
            findings.push(Finding {
                id: stable_id("finding", &format!("{id}:{title}")),
                title: title.into(),
                body: decode_basic_entities(&snippet),
                facet: best_facet(plan),
                confidence: Confidence::Low,
                source_ids: vec![id.clone()],
                content_trust: ContentTrust::ExternalUntrusted,
                tags,
            });
            sources.push(SourceRecord {
                id,
                title: format!("Wikipedia: {title}"),
                url,
                publisher: "Wikimedia Foundation".into(),
                retrieved_at: now,
                published_at: None,
                license: Some("CC BY-SA 4.0; page-specific attribution applies".into()),
                source_type: SourceType::Encyclopedia,
                quality: SourceQuality::DiscoveryOnly,
                content_hash: Some(hash_text(&snippet)),
                provenance: ProvenanceDetails {
                    request_url: Some(request_url.to_string()),
                    ..Default::default()
                },
            });
        }
        Ok(SourceOutput {
            connector: self.name().into(),
            findings,
            metrics: Vec::new(),
            sources,
            warnings: Vec::new(),
            audit: ConnectorAudit::default(),
        })
    }
}

pub struct OpenAlexSource {
    client: Client,
}

pub struct Nasa3dSource {
    client: Client,
}

pub struct WikimediaCommonsSource {
    client: Client,
}

impl WikimediaCommonsSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PublicSource for WikimediaCommonsSource {
    fn name(&self) -> &'static str {
        "Wikimedia Commons media"
    }

    fn supports(&self, plan: &ResearchPlan) -> bool {
        plan.facets.contains(&Facet::Assets)
            && is_image_request(&plan.query)
            && current_office_search_phrase(&plan.query).is_none()
            && !office_query_needs_jurisdiction(&plan.query)
    }

    async fn search(&self, plan: &ResearchPlan, limit: usize) -> Result<SourceOutput> {
        let endpoint = "https://commons.wikimedia.org/w/api.php";
        let mut request_url = Url::parse(endpoint)?;
        request_url
            .query_pairs_mut()
            .append_pair("action", "query")
            .append_pair("format", "json")
            .append_pair("formatversion", "2")
            .append_pair("generator", "search")
            .append_pair("gsrsearch", &commons_search_query(&plan.query))
            .append_pair("gsrnamespace", "6")
            .append_pair("gsrlimit", &limit.min(5).to_string())
            .append_pair("prop", "imageinfo")
            .append_pair("iiprop", "url|mime|size|sha1|extmetadata")
            .append_pair(
                "iiextmetadatafilter",
                "LicenseShortName|LicenseUrl|Artist|ImageDescription|Credit",
            )
            .append_pair("iiurlwidth", "640");
        let response = self
            .client
            .get(request_url.clone())
            .send()
            .await?
            .error_for_status()?;
        let response = bytes_limited(response, 4_000_000, "Wikimedia Commons").await?;
        let root: Value = serde_json::from_slice(&response)?;
        let mut pages = root
            .pointer("/query/pages")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        pages.sort_by_key(|page| {
            page.get("index")
                .and_then(Value::as_i64)
                .unwrap_or(i64::MAX)
        });
        let now = Utc::now();
        let mut findings = Vec::new();
        let mut sources = Vec::new();
        for page in pages.into_iter().take(limit.min(5)) {
            let Some(info) = page.pointer("/imageinfo/0") else {
                continue;
            };
            let title = page
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Untitled Commons file")
                .trim_start_matches("File:")
                .to_owned();
            let Some(description_url) = info
                .get("descriptionurl")
                .and_then(Value::as_str)
                .and_then(|value| {
                    strict_media_https_url(Some(value), &["commons.wikimedia.org"]).ok()
                })
            else {
                continue;
            };
            let content_url = info.get("url").and_then(Value::as_str).and_then(|value| {
                strict_media_https_url(Some(value), &["upload.wikimedia.org"]).ok()
            });
            let preview_url = info
                .get("thumburl")
                .and_then(Value::as_str)
                .and_then(|value| {
                    strict_media_https_url(Some(value), &["upload.wikimedia.org"]).ok()
                });
            let mime = info.get("mime").and_then(Value::as_str).map(str::to_owned);
            if is_image_request(&plan.query)
                && !mime
                    .as_deref()
                    .is_some_and(|value| value.starts_with("image/"))
            {
                continue;
            }
            let size = info.get("size").and_then(Value::as_u64);
            let description = commons_metadata(info, "ImageDescription")
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "No machine-readable description was supplied.".into());
            let artist = commons_metadata(info, "Artist")
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "creator not supplied in machine-readable metadata".into());
            let license_name = commons_metadata(info, "LicenseShortName")
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "license not supplied in machine-readable metadata".into());
            let license_url = commons_metadata(info, "LicenseUrl").unwrap_or_default();
            if !recognized_commons_license(
                &license_name,
                (!license_url.is_empty()).then_some(license_url.as_str()),
            ) {
                continue;
            }
            let license = if safe_http_url(&license_url).is_some() {
                format!("{license_name}; {license_url}")
            } else {
                license_name.clone()
            };
            if !media_record_relevant(&plan.query, &format!("{title} {description}")) {
                continue;
            }
            let id = stable_id("commons", &description_url);
            let body = truncate_chars(
                &format!(
                    "{description} Creator/credit: {artist}. File type: {}. Size: {}. Requested labels, structures, viewpoint, recency, and other visual features were not independently verified; inspect the preview and Commons description before use. Verify subject identity, attribution, license, and any graphic-content warning.",
                    mime.as_deref().unwrap_or("not supplied"),
                    size.map(|value| format!("{value} bytes"))
                        .unwrap_or_else(|| "not supplied".into())
                ),
                900,
            );
            findings.push(Finding {
                id: stable_id("finding", &format!("{id}:{title}")),
                title: title.clone(),
                body: body.clone(),
                facet: Facet::Assets,
                confidence: Confidence::Low,
                source_ids: vec![id.clone()],
                content_trust: ContentTrust::ExternalUntrusted,
                tags: vec![
                    "open-media".into(),
                    mime.clone().unwrap_or_else(|| "format-unknown".into()),
                    license_name,
                ],
            });
            sources.push(SourceRecord {
                id,
                title: title.clone(),
                url: description_url,
                publisher: "Wikimedia Commons contributor community".into(),
                retrieved_at: now,
                published_at: None,
                license: Some(license),
                source_type: SourceType::Other,
                quality: SourceQuality::DiscoveryOnly,
                content_hash: info
                    .get("sha1")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .or_else(|| Some(hash_text(&body))),
                provenance: ProvenanceDetails {
                    dataset_id: Some("Wikimedia Commons File namespace".into()),
                    request_url: Some(request_url.to_string()),
                    methodology_url: Some(
                        "https://commons.wikimedia.org/wiki/Commons:Machine-readable_data".into(),
                    ),
                    content_url,
                    preview_url,
                    file_format: mime,
                    file_size_bytes: size,
                    creator: Some(artist.clone()),
                    credit: commons_metadata(info, "Credit"),
                    license_url: safe_http_url(&license_url).map(str::to_owned),
                    alt_text: Some(title.clone()),
                    media_role: Some("rights_checked_event_media".into()),
                    ..Default::default()
                },
            });
        }
        Ok(SourceOutput {
            connector: self.name().into(),
            findings,
            metrics: Vec::new(),
            sources,
            warnings: Vec::new(),
            audit: ConnectorAudit::default(),
        })
    }
}

fn commons_metadata(info: &Value, key: &str) -> Option<String> {
    info.pointer(&format!("/extmetadata/{key}/value"))
        .and_then(Value::as_str)
        .map(strip_markup)
}

fn recognized_commons_license(name: &str, url: Option<&str>) -> bool {
    let normalized = normalize_for_matching(name);
    if contains_phrase(&normalized, "public domain") || contains_phrase(&normalized, "cc0") {
        return true;
    }
    let Some(parsed) = url.and_then(|value| Url::parse(value).ok()) else {
        return false;
    };
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("creativecommons.org")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return false;
    }
    let path = parsed.path().trim_end_matches('/');
    let named_version = name.split_whitespace().find_map(|token| {
        let token =
            token.trim_matches(|character: char| !character.is_ascii_digit() && character != '.');
        (token.chars().any(|character| character == '.')
            && token
                .chars()
                .all(|character| character.is_ascii_digit() || character == '.'))
        .then_some(token)
    });
    let url_version = path.rsplit('/').next();
    if named_version.is_none() || named_version != url_version {
        return false;
    }
    if contains_phrase(&normalized, "cc by sa") {
        path.starts_with("/licenses/by-sa/")
    } else if contains_phrase(&normalized, "cc by") {
        path.starts_with("/licenses/by/")
    } else {
        false
    }
}

fn current_uk_monarch_from_parliament_csv(
    reigns_csv: &str,
    monarchs_csv: &str,
) -> Result<UkParliamentMonarchRecord> {
    let mut reign_lines = reigns_csv
        .lines()
        .map(|line| line.trim_end_matches('\r').trim())
        .filter(|line| !line.is_empty());
    let reign_header = reign_lines
        .next()
        .map(|line| line.trim_start_matches('\u{feff}'))
        .ok_or_else(|| anyhow!("UK Parliament reigns CSV was empty"))?;
    if reign_header != "Start date,End date,Kingdom,Monarch" {
        bail!("UK Parliament reigns CSV schema changed");
    }
    let mut current_rows = Vec::new();
    for line in reign_lines {
        if line.contains('"') {
            bail!("UK Parliament reigns CSV introduced unsupported quoting");
        }
        let columns = line.split(',').map(str::trim).collect::<Vec<_>>();
        if columns.len() != 4 {
            bail!("UK Parliament reigns CSV row did not have four columns");
        }
        if columns[1].is_empty() && columns[2] == "United Kingdom" {
            current_rows.push((columns[0], columns[3]));
        }
    }
    if current_rows.len() != 1 {
        bail!(
            "UK Parliament reigns CSV had {} current United Kingdom rows; Inquiry abstained",
            current_rows.len()
        );
    }
    let (reign_started, name) = current_rows[0];
    let start_date = chrono::NaiveDate::parse_from_str(reign_started, "%Y-%m-%d")
        .context("UK Parliament current reign had an invalid start date")?;
    if start_date > Utc::now().date_naive() {
        bail!("UK Parliament current reign starts in the future");
    }

    let mut monarch_lines = monarchs_csv
        .lines()
        .map(|line| line.trim_end_matches('\r').trim())
        .filter(|line| !line.is_empty());
    let monarch_header = monarch_lines
        .next()
        .map(|line| line.trim_start_matches('\u{feff}'))
        .ok_or_else(|| anyhow!("UK Parliament monarchs CSV was empty"))?;
    if monarch_header != "Title,Date of birth,Date of death,Wikidata ID" {
        bail!("UK Parliament monarchs CSV schema changed");
    }
    let mut matching_monarchs = Vec::new();
    for line in monarch_lines {
        if line.contains('"') {
            bail!("UK Parliament monarchs CSV introduced unsupported quoting");
        }
        let columns = line.split(',').map(str::trim).collect::<Vec<_>>();
        if columns.len() != 4 {
            bail!("UK Parliament monarchs CSV row did not have four columns");
        }
        if normalize_for_matching(columns[0]) == normalize_for_matching(name)
            && columns[2].is_empty()
        {
            matching_monarchs.push((columns[1], columns[3]));
        }
    }
    if matching_monarchs.len() != 1 {
        bail!(
            "UK Parliament monarchs CSV had {} living records matching the current reign; Inquiry abstained",
            matching_monarchs.len()
        );
    }
    let (birth_date, wikidata_id) = matching_monarchs[0];
    chrono::NaiveDate::parse_from_str(birth_date, "%Y-%m-%d")
        .context("UK Parliament current monarch had an invalid birth date")?;
    if !wikidata_id.starts_with('Q')
        || !wikidata_id[1..]
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        bail!("UK Parliament current monarch had an invalid Wikidata identifier");
    }

    Ok(UkParliamentMonarchRecord {
        name: name.into(),
        reign_started: reign_started.into(),
        birth_date: birth_date.into(),
        wikidata_id: wikidata_id.into(),
    })
}

fn royal_family_identity_summary(html: &str, person_name: &str) -> Option<String> {
    let patterns = [
        r#"(?is)<meta[^>]+(?:name|property)=[\"'](?:description|og:description)[\"'][^>]+content=[\"']([^\"']+)[\"']"#,
        r#"(?is)<meta[^>]+content=[\"']([^\"']+)[\"'][^>]+(?:name|property)=[\"'](?:description|og:description)[\"']"#,
    ];
    patterns.into_iter().find_map(|pattern| {
        Regex::new(pattern)
            .expect("valid Royal Family identity regex")
            .captures(html)
            .and_then(|captures| captures.get(1))
            .map(|matched| decode_basic_entities(&strip_markup(matched.as_str())))
            .filter(|summary| {
                person_name_tokens_match(summary, person_name)
                    && contains_phrase(summary, "became king")
            })
    })
}

fn human_readable_iso_date(value: &str) -> Option<String> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .map(|date| date.format("%-d %B %Y").to_string())
}

fn usagov_current_president_section(html: &str) -> Option<&str> {
    Regex::new(
        r#"(?is)<h[23][^>]*>\s*Current\s+president\s*</h[23]>(?P<section>.*?)(?:<h[23][^>]*>|$)"#,
    )
    .expect("valid USAGov section regex")
    .captures(html)
    .and_then(|captures| captures.name("section"))
    .map(|matched| matched.as_str())
}

fn person_name_tokens_match(normalized_text: &str, person_name: &str) -> bool {
    let normalized_text = normalize_for_matching(normalized_text);
    let tokens = normalize_for_matching(person_name)
        .split_whitespace()
        .filter(|token| token.chars().count() > 1)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    tokens.len() >= 2
        && tokens
            .iter()
            .all(|token| contains_phrase(&normalized_text, token))
}

fn white_house_identity_title(html: &str) -> Option<String> {
    let patterns = [
        r#"(?is)<meta[^>]+property=[\"']og:title[\"'][^>]+content=[\"']([^\"']+)[\"']"#,
        r#"(?is)<meta[^>]+content=[\"']([^\"']+)[\"'][^>]+property=[\"']og:title[\"']"#,
        r#"(?is)<h1[^>]*>(.*?)</h1>"#,
        r#"(?is)<title[^>]*>(.*?)</title>"#,
    ];
    patterns.into_iter().find_map(|pattern| {
        Regex::new(pattern)
            .expect("valid White House identity regex")
            .captures(html)
            .and_then(|captures| captures.get(1))
            .map(|matched| strip_markup(matched.as_str()))
            .filter(|value| !value.trim().is_empty())
    })
}

fn compact_source_error(error: &anyhow::Error) -> String {
    error.to_string().chars().take(220).collect()
}

impl Nasa3dSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PublicSource for Nasa3dSource {
    fn name(&self) -> &'static str {
        "NASA 3D Resources"
    }

    fn supports(&self, plan: &ResearchPlan) -> bool {
        plan.facets.contains(&Facet::Assets) && is_3d_asset_request(&plan.query)
    }

    async fn search(&self, plan: &ResearchPlan, limit: usize) -> Result<SourceOutput> {
        let request_url = "https://science.nasa.gov/3d-resources/";
        let response = self
            .client
            .get(request_url)
            .send()
            .await?
            .error_for_status()?;
        let response = bytes_limited(response, 4_000_000, "NASA 3D Resources").await?;
        let html =
            std::str::from_utf8(&response).context("NASA 3D Resources returned non-UTF-8 HTML")?;
        let matches = find_nasa_3d_links(html, &plan.query, limit.min(8));
        let now = Utc::now();
        let page_hash = hash_text(html);
        let mut findings = Vec::new();
        let mut sources = Vec::new();
        for (title, url) in matches {
            let id = stable_id("nasa3d", &url);
            findings.push(Finding {
                id: stable_id("finding", &format!("{id}:{title}")),
                title: title.clone(),
                body: "Matching record in NASA's official 3D Resources catalog. Open the resource page to inspect available formats, scale, print suitability, attribution, and reuse terms before downloading.".into(),
                facet: Facet::Assets,
                confidence: Confidence::Moderate,
                source_ids: vec![id.clone()],
                content_trust: ContentTrust::CuratedTemplate,
                tags: vec!["3d-asset".into(), "official-catalog-match".into()],
            });
            sources.push(SourceRecord {
                id,
                title,
                url,
                publisher: "NASA Science".into(),
                retrieved_at: now,
                published_at: None,
                license: Some("NASA media usage guidelines and resource-specific terms apply; verify before reuse".into()),
                source_type: SourceType::Government,
                quality: SourceQuality::StrongSecondary,
                content_hash: Some(page_hash.clone()),
                provenance: ProvenanceDetails {
                    dataset_id: Some("NASA 3D Resources catalog".into()),
                    request_url: Some(request_url.into()),
                    methodology_url: Some("https://www.nasa.gov/nasa-brand-center/images-and-media/".into()),
                    ..Default::default()
                },
            });
        }
        Ok(SourceOutput {
            connector: self.name().into(),
            findings,
            metrics: Vec::new(),
            sources,
            warnings: Vec::new(),
            audit: ConnectorAudit::default(),
        })
    }
}

fn find_nasa_3d_links(html: &str, query: &str, limit: usize) -> Vec<(String, String)> {
    let generic = [
        "3d",
        "model",
        "models",
        "print",
        "printing",
        "file",
        "files",
        "find",
        "download",
        "nasa",
        "official",
        "printable",
        "resource",
        "resources",
        "of",
        "the",
        "a",
        "an",
        "for",
    ];
    let terms = normalize_for_matching(query)
        .split_whitespace()
        .filter(|term| term.chars().count() >= 2 && !generic.contains(term))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if terms.is_empty() {
        return Vec::new();
    }
    let links =
        Regex::new(r#"href=[\"'](https://science\.nasa\.gov/3d-resources/([a-z0-9-]+)/?)[\"']"#)
            .expect("valid NASA 3D link regex");
    let mut scored = BTreeMap::<String, (usize, String)>::new();
    for captures in links.captures_iter(html) {
        let Some(url) = captures.get(1).map(|value| value.as_str().to_owned()) else {
            continue;
        };
        let Some(slug) = captures.get(2).map(|value| value.as_str()) else {
            continue;
        };
        let slug_terms = slug.split('-').collect::<BTreeSet<_>>();
        let score = terms
            .iter()
            .filter(|term| slug_terms.contains(term.as_str()))
            .count();
        if score == 0 {
            continue;
        }
        let title = slug
            .split('-')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut characters = part.chars();
                characters
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ");
        scored
            .entry(url)
            .and_modify(|existing| existing.0 = existing.0.max(score))
            .or_insert((score, title));
    }
    let mut matches = scored
        .into_iter()
        .map(|(url, (score, title))| (score, title, url))
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    let best_score = matches.first().map(|item| item.0).unwrap_or_default();
    let required_score = terms.len().saturating_mul(2).div_ceil(3).max(1);
    matches
        .into_iter()
        .filter(|(score, _, _)| *score == best_score && *score >= required_score)
        .take(limit)
        .map(|(_, title, url)| (title, url))
        .collect()
}

pub struct MedlinePlusSource {
    client: Client,
}

impl MedlinePlusSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PublicSource for MedlinePlusSource {
    fn name(&self) -> &'static str {
        "MedlinePlus"
    }

    fn supports(&self, plan: &ResearchPlan) -> bool {
        if is_scholarly_request(&plan.query) {
            return false;
        }
        plan.facets.iter().any(|facet| {
            matches!(
                facet,
                Facet::Health | Facet::Transmission | Facet::Psychology
            )
        })
    }

    async fn search(&self, plan: &ResearchPlan, limit: usize) -> Result<SourceOutput> {
        let endpoint = "https://wsearch.nlm.nih.gov/ws/query";
        let mut request_url = Url::parse(endpoint)?;
        request_url
            .query_pairs_mut()
            .append_pair("db", "healthTopics")
            .append_pair("term", &health_subject_query(&plan.query))
            .append_pair("rettype", "brief")
            .append_pair("retmax", &limit.min(5).to_string())
            .append_pair("tool", "barnlabs_inquiry");
        let response = self
            .client
            .get(request_url.clone())
            .send()
            .await?
            .error_for_status()?;
        let response = bytes_limited(response, 2_000_000, "MedlinePlus").await?;
        let documents = parse_medlineplus(&response)?;
        let subject = normalize_for_matching(&health_subject_query(&plan.query));
        let has_exact_subject = documents
            .iter()
            .any(|document| normalize_for_matching(&document.title) == subject);
        let now = Utc::now();
        let mut findings = Vec::new();
        let mut sources = Vec::new();
        for document in documents.into_iter().take(limit.min(5)) {
            if document.title.is_empty() || safe_http_url(&document.url).is_none() {
                continue;
            }
            if has_exact_subject && normalize_for_matching(&document.title) != subject {
                continue;
            }
            if !query_record_relevant(
                &health_subject_query(&plan.query),
                &format!("{} {}", document.title, document.snippet),
            ) {
                continue;
            }
            let id = stable_id("medlineplus", &document.url);
            let body = if document.snippet.is_empty() {
                "Official MedlinePlus health-topic result. Open the source for the reviewed topic summary and linked guidance.".into()
            } else {
                truncate_chars(&document.snippet, 600)
            };
            findings.push(Finding {
                id: stable_id("finding", &format!("{id}:{}", document.title)),
                title: document.title.clone(),
                body: body.clone(),
                facet: Facet::Health,
                confidence: Confidence::Moderate,
                source_ids: vec![id.clone()],
                content_trust: ContentTrust::ExternalUntrusted,
                tags: vec!["official-health-topic".into(), "consumer-health".into()],
            });
            sources.push(SourceRecord {
                id,
                title: document.title,
                url: document.url,
                publisher: if document.organization.is_empty() {
                    "MedlinePlus, U.S. National Library of Medicine".into()
                } else {
                    document.organization
                },
                retrieved_at: now,
                published_at: None,
                license: Some("MedlinePlus Web service: free with MedlinePlus.gov attribution; linked content terms can vary".into()),
                source_type: SourceType::Government,
                quality: SourceQuality::StrongSecondary,
                content_hash: Some(hash_text(&body)),
                provenance: ProvenanceDetails {
                    dataset_id: Some("MedlinePlus healthTopics".into()),
                    request_url: Some(request_url.to_string()),
                    methodology_url: Some("https://medlineplus.gov/about/developers/webservices/".into()),
                    ..Default::default()
                },
            });
        }
        Ok(SourceOutput {
            connector: self.name().into(),
            findings,
            metrics: Vec::new(),
            sources,
            warnings: Vec::new(),
            audit: ConnectorAudit::default(),
        })
    }
}

#[derive(Default)]
struct MedlineDocument {
    url: String,
    title: String,
    snippet: String,
    organization: String,
}

fn parse_medlineplus(xml: &[u8]) -> Result<Vec<MedlineDocument>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut documents = Vec::new();
    let mut current: Option<MedlineDocument> = None;
    let mut field: Option<String> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) if start.name().as_ref() == b"document" => {
                let mut document = MedlineDocument::default();
                for attribute in start.attributes().flatten() {
                    if attribute.key.as_ref() == b"url" {
                        document.url = attribute
                            .decoded_and_normalized_value(XmlVersion::default(), reader.decoder())?
                            .into_owned();
                    }
                }
                current = Some(document);
            }
            Ok(Event::Start(start)) if start.name().as_ref() == b"content" => {
                field = start
                    .attributes()
                    .flatten()
                    .find(|attribute| attribute.key.as_ref() == b"name")
                    .map(|attribute| {
                        attribute
                            .decoded_and_normalized_value(XmlVersion::default(), reader.decoder())
                            .map(|value| value.into_owned())
                    })
                    .transpose()?;
            }
            Ok(Event::Text(text)) => {
                if let (Some(document), Some(field_name)) = (&mut current, &field) {
                    let decoded = text.decode()?;
                    let value = quick_xml::escape::unescape(&decoded)?.into_owned();
                    let target = match field_name.as_str() {
                        "title" => Some(&mut document.title),
                        "snippet" => Some(&mut document.snippet),
                        "organizationName" => Some(&mut document.organization),
                        _ => None,
                    };
                    if let Some(target) = target {
                        if !target.is_empty() && !value.trim().is_empty() {
                            target.push(' ');
                        }
                        target.push_str(value.trim());
                    }
                }
            }
            Ok(Event::End(end)) if end.name().as_ref() == b"content" => field = None,
            Ok(Event::End(end)) if end.name().as_ref() == b"document" => {
                if let Some(mut document) = current.take() {
                    document.title = strip_markup(&document.title);
                    document.snippet = strip_markup(&document.snippet);
                    document.organization = strip_markup(&document.organization);
                    documents.push(document);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(error).context("MedlinePlus returned malformed XML"),
            _ => {}
        }
    }
    Ok(documents)
}
impl OpenAlexSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PublicSource for OpenAlexSource {
    fn name(&self) -> &'static str {
        "OpenAlex"
    }
    fn supports(&self, plan: &ResearchPlan) -> bool {
        let scholarly_intent = is_scholarly_request(&plan.query);
        if is_image_request(&plan.query) && !scholarly_intent {
            return false;
        }
        plan.facets.iter().any(|f| {
            matches!(
                f,
                Facet::Health
                    | Facet::Transmission
                    | Facet::Formulas
                    | Facet::Statistics
                    | Facet::Textbooks
                    | Facet::Psychology
                    | Facet::Science
            )
        }) || scholarly_intent
    }

    async fn search(&self, plan: &ResearchPlan, limit: usize) -> Result<SourceOutput> {
        let cleaned_query = clean_openalex_query(&plan.query);
        let recent = query_has_any_word(&plan.query, &["recent", "latest", "newest"]);
        let today = Utc::now().date_naive();
        let recent_from = format!("{}-01-01", today.year() - 5);
        let mut request_url = Url::parse("https://api.openalex.org/works")?;
        {
            let mut pairs = request_url.query_pairs_mut();
            pairs
                .append_pair("search", &cleaned_query)
                .append_pair("per-page", &limit.min(5).to_string())
                .append_pair(
                    "select",
                    "id,display_name,publication_year,publication_date,doi,primary_location,cited_by_count",
                );
            if recent {
                pairs.append_pair(
                    "filter",
                    &format!("from_publication_date:{recent_from},to_publication_date:{today}"),
                );
            }
        }
        let response = self
            .client
            .get(request_url.clone())
            .send()
            .await?
            .error_for_status()?;
        let response: Value = json_limited(response, 4_000_000, "OpenAlex").await?;
        let works = response
            .get("results")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("OpenAlex response did not include results"))?;
        let now = Utc::now();
        let mut findings = Vec::new();
        let mut sources = Vec::new();
        let facet = best_facet(plan);
        for work in works {
            let title = work
                .get("display_name")
                .and_then(Value::as_str)
                .unwrap_or("Untitled work");
            if !query_record_relevant(&plan.query, title) {
                continue;
            }
            let year = work
                .get("publication_year")
                .and_then(Value::as_i64)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "date unknown".into());
            let publication_date = work
                .get("publication_date")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| year.clone());
            let cited = work
                .get("cited_by_count")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            let url = work
                .get("doi")
                .and_then(Value::as_str)
                .or_else(|| work.get("id").and_then(Value::as_str))
                .unwrap_or("https://openalex.org")
                .to_string();
            let id = stable_id("openalex", &url);
            findings.push(Finding { id: stable_id("finding", &format!("{id}:{title}")), title: title.into(), body: format!("Discovery metadata for a scholarly work published in {year}; OpenAlex reports {cited} citing works. This is not a verified claim from the paper. Inspect the original work before relying on its conclusions."), facet, confidence: Confidence::Low, source_ids: vec![id.clone()], content_trust: ContentTrust::ExternalUntrusted, tags: vec!["scholarly-metadata".into(), year.clone()] });
            sources.push(SourceRecord {
                id,
                title: title.into(),
                url,
                publisher: "OpenAlex (metadata); original publisher at linked record".into(),
                retrieved_at: now,
                published_at: Some(publication_date.clone()),
                license: Some("OpenAlex data: CC0; linked work license varies".into()),
                source_type: SourceType::SearchIndex,
                quality: SourceQuality::DiscoveryOnly,
                content_hash: None,
                provenance: ProvenanceDetails {
                    dataset_id: Some("OpenAlex Works".into()),
                    request_url: Some(request_url.to_string()),
                    observation_period: Some(publication_date),
                    ..Default::default()
                },
            });
        }
        let mut warnings = Vec::new();
        if recent {
            warnings.push(format!(
                "OpenAlex 'recent' discovery was limited to {recent_from} through {today}; indexing and publication dates can lag or change."
            ));
        }
        if contains_phrase(&plan.query, "effect size") {
            warnings.push("OpenAlex supplies discovery metadata, not effect sizes. Extract and independently verify statistical estimates from each original paper before synthesis.".into());
        }
        Ok(SourceOutput {
            connector: self.name().into(),
            findings,
            metrics: Vec::new(),
            sources,
            warnings,
            audit: ConnectorAudit::default(),
        })
    }
}

pub struct OpenLibrarySource {
    client: Client,
}
impl OpenLibrarySource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PublicSource for OpenLibrarySource {
    fn name(&self) -> &'static str {
        "Open Library"
    }
    fn supports(&self, plan: &ResearchPlan) -> bool {
        plan.facets.contains(&Facet::Textbooks)
    }

    async fn search(&self, plan: &ResearchPlan, limit: usize) -> Result<SourceOutput> {
        let mut request_url = Url::parse("https://openlibrary.org/search.json")?;
        request_url
            .query_pairs_mut()
            .append_pair("q", &plan.query)
            .append_pair("limit", &limit.min(8).to_string())
            .append_pair(
                "fields",
                "key,title,author_name,first_publish_year,ebook_access,public_scan_b",
            );
        let response = self
            .client
            .get(request_url.clone())
            .send()
            .await?
            .error_for_status()?;
        let response: Value = json_limited(response, 4_000_000, "Open Library").await?;
        let docs = response
            .get("docs")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("Open Library response did not include docs"))?;
        let now = Utc::now();
        let mut findings = Vec::new();
        let mut sources = Vec::new();
        for doc in docs {
            let title = doc
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Untitled book");
            if !query_record_relevant(&plan.query, title) {
                continue;
            }
            let key = doc.get("key").and_then(Value::as_str).unwrap_or("");
            let url = format!("https://openlibrary.org{key}");
            let authors = doc
                .get("author_name")
                .and_then(Value::as_array)
                .map(|xs| {
                    xs.iter()
                        .filter_map(Value::as_str)
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_else(|| "Author unknown".into());
            let access = doc
                .get("ebook_access")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let public_scan = doc
                .get("public_scan_b")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let id = stable_id("openlibrary", &url);
            findings.push(Finding { id: stable_id("finding", &format!("{id}:{title}")), title: title.into(), body: format!("Discovery metadata: by {authors}. Ebook access: {access}. Public scan indicated: {public_scan}. Availability is not the same as an open license; confirm rights on the item page."), facet: Facet::Textbooks, confidence: Confidence::Low, source_ids: vec![id.clone()], content_trust: ContentTrust::ExternalUntrusted, tags: vec!["book-metadata".into(), access.into()] });
            sources.push(SourceRecord {
                id,
                title: format!("Open Library record: {title}"),
                url,
                publisher: "Internet Archive / Open Library".into(),
                retrieved_at: now,
                published_at: doc
                    .get("first_publish_year")
                    .and_then(Value::as_i64)
                    .map(|v| v.to_string()),
                license: Some(
                    "Catalog metadata is open; item copyright and borrowing terms vary".into(),
                ),
                source_type: SourceType::OpenEducation,
                quality: SourceQuality::DiscoveryOnly,
                content_hash: None,
                provenance: ProvenanceDetails {
                    dataset_id: Some("Open Library Search".into()),
                    request_url: Some(request_url.to_string()),
                    ..Default::default()
                },
            });
        }
        Ok(SourceOutput {
            connector: self.name().into(),
            findings,
            metrics: Vec::new(),
            sources,
            warnings: Vec::new(),
            audit: ConnectorAudit::default(),
        })
    }
}

pub struct WorldBankSource {
    client: Client,
}
impl WorldBankSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PublicSource for WorldBankSource {
    fn name(&self) -> &'static str {
        "World Bank Open Data"
    }
    fn supports(&self, plan: &ResearchPlan) -> bool {
        plan.facets
            .iter()
            .any(|f| matches!(f, Facet::Financials | Facet::Locations | Facet::Statistics))
    }

    async fn search(&self, plan: &ResearchPlan, _: usize) -> Result<SourceOutput> {
        reject_ambiguous_country_query(&plan.query)?;
        let mut country_catalog_url = Url::parse("https://api.worldbank.org/v2/country")?;
        country_catalog_url
            .query_pairs_mut()
            .append_pair("format", "json")
            .append_pair("per_page", "400");
        let country_response = self
            .client
            .get(country_catalog_url)
            .send()
            .await?
            .error_for_status()?;
        let countries: Value =
            json_limited(country_response, 4_000_000, "World Bank country catalog").await?;
        let list = countries
            .get(1)
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("World Bank country list was unavailable"))?;
        let mut countries = list
            .iter()
            .filter_map(|c| {
                let name = c.get("name")?.as_str()?;
                let iso2 = c.get("iso2Code")?.as_str()?;
                let id = c.get("id")?.as_str()?;
                let matched = country_query_matches(&plan.query, name);
                matched.then_some((name, iso2, id))
            })
            .collect::<Vec<_>>();
        countries.sort_by_key(|(name, _, _)| name.to_lowercase());
        countries.dedup_by_key(|(_, _, id)| *id);
        if countries.is_empty() {
            return Err(anyhow!(
                "no country in the query matched the World Bank country catalog"
            ));
        }
        if countries.len() > 4 {
            bail!(
                "the query matched {} World Bank countries or regions; narrow the comparison to at most four explicit countries",
                countries.len()
            );
        }

        let indicators = [
            ("SP.POP.TOTL", "Population", "people", Facet::Locations),
            (
                "NY.GDP.MKTP.CD",
                "GDP (current US$)",
                "USD",
                Facet::Financials,
            ),
            (
                "NY.GDP.PCAP.CD",
                "GDP per capita (current US$)",
                "USD/person",
                Facet::Financials,
            ),
            (
                "SP.DYN.LE00.IN",
                "Life expectancy at birth",
                "years",
                Facet::Health,
            ),
        ];
        let mut metrics = Vec::new();
        let mut findings = Vec::new();
        let mut sources = Vec::new();
        let mut connector_warnings = Vec::new();
        let now = Utc::now();
        for country in countries {
            let country_source_start = sources.len();
            for (code, label, unit, facet) in indicators {
                let api = format!(
                    "https://api.worldbank.org/v2/country/{}/indicator/{code}",
                    country.2
                );
                let mut request_url = Url::parse(&api)?;
                request_url
                    .query_pairs_mut()
                    .append_pair("format", "json")
                    .append_pair("per_page", "10")
                    .append_pair("mrv", "10");
                let indicator_response = match self
                    .client
                    .get(request_url.clone())
                    .send()
                    .await
                    .and_then(reqwest::Response::error_for_status)
                {
                    Ok(response) => response,
                    Err(error) => {
                        connector_warnings.push(format!(
                            "World Bank did not return {label} for {}: {error}",
                            country.0
                        ));
                        continue;
                    }
                };
                let response: Value =
                    match json_limited(indicator_response, 2_000_000, "World Bank indicator").await
                    {
                        Ok(response) => response,
                        Err(error) => {
                            connector_warnings.push(format!(
                                "World Bank returned unusable {label} data for {}: {error}",
                                country.0
                            ));
                            continue;
                        }
                    };
                let Some(row) = response.get(1).and_then(Value::as_array).and_then(|rows| {
                    rows.iter()
                        .find(|row| !row.get("value").unwrap_or(&Value::Null).is_null())
                }) else {
                    continue;
                };
                let Some(value) = row.get("value").and_then(Value::as_f64) else {
                    continue;
                };
                let period = row
                    .get("date")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let url = format!(
                    "https://data.worldbank.org/indicator/{code}?locations={}",
                    country.1
                );
                let id = stable_id("worldbank", &format!("{url}:{period}:{value:.17}"));
                metrics.push(Metric {
                    label: format!("{} — {}", country.0, label),
                    value,
                    display_value: human_number(value),
                    unit: unit.into(),
                    facet,
                    source_ids: vec![id.clone()],
                    period: Some(period.clone()),
                });
                sources.push(SourceRecord {
                    id,
                    title: format!("{label} for {} ({period})", country.0),
                    url,
                    publisher: "World Bank".into(),
                    retrieved_at: now,
                    published_at: None,
                    license: Some(
                        "World Bank datasets: CC BY 4.0 unless dataset states otherwise".into(),
                    ),
                    source_type: SourceType::Intergovernmental,
                    quality: SourceQuality::Primary,
                    content_hash: Some(hash_text(&serde_json::to_string(row)?)),
                    provenance: ProvenanceDetails {
                        dataset_id: Some(code.into()),
                        request_url: Some(request_url.to_string()),
                        methodology_url: Some(format!(
                            "https://data.worldbank.org/indicator/{code}"
                        )),
                        observation_period: Some(period),
                        source_updated_at: None,
                        ..Default::default()
                    },
                });
            }
            let source_ids = sources[country_source_start..]
                .iter()
                .map(|source| source.id.clone())
                .collect::<Vec<_>>();
            if !source_ids.is_empty() {
                let title = format!("World Bank profile: {}", country.0);
                findings.push(Finding { id: stable_id("finding", &format!("worldbank:{title}")), title, body: "The metrics below use the most recent non-null observations returned for each indicator. Periods can differ across indicators and countries, and later revisions are possible.".into(), facet: Facet::Statistics, confidence: Confidence::High, source_ids, content_trust: ContentTrust::CuratedTemplate, tags: vec!["official-statistics".into()] });
            } else {
                connector_warnings.push(format!(
                    "World Bank returned no usable requested indicators for {}.",
                    country.0
                ));
            }
        }
        Ok(SourceOutput {
            connector: self.name().into(),
            findings,
            metrics,
            sources,
            warnings: connector_warnings,
            audit: ConnectorAudit::default(),
        })
    }
}

pub struct SearxngSource {
    client: Client,
    endpoint: String,
}
impl SearxngSource {
    pub fn new(client: Client, endpoint: String) -> Result<Self> {
        let endpoint = validate_searxng_endpoint(&endpoint)?;
        Ok(Self { client, endpoint })
    }
}

fn validate_searxng_endpoint(value: &str) -> Result<String> {
    let mut parsed = Url::parse(value.trim()).context("SearXNG endpoint must be a valid URL")?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        bail!("SearXNG endpoint must not contain embedded credentials");
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        bail!("SearXNG endpoint must not contain a query string or fragment");
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("SearXNG endpoint must include a host"))?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    match parsed.scheme() {
        "https" => {
            if host.ends_with(".local")
                || host.ends_with(".internal")
                || host.parse::<IpAddr>().is_ok_and(is_non_public_ip)
            {
                bail!("SearXNG endpoint must not target a private or link-local network address");
            }
        }
        "http" if loopback => {}
        "http" => bail!("SearXNG requires HTTPS; HTTP is allowed only for loopback development"),
        _ => bail!("SearXNG endpoint must use HTTPS, or HTTP on loopback for development"),
    }
    parsed.set_query(None);
    parsed.set_fragment(None);
    Ok(parsed.as_str().trim_end_matches('/').to_owned())
}

fn is_non_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            address.is_private()
                || address.is_link_local()
                || address.is_unspecified()
                || address.is_broadcast()
                || address.is_documentation()
                || address.is_multicast()
        }
        IpAddr::V6(address) => {
            address.is_unique_local()
                || address.is_unicast_link_local()
                || address.is_unspecified()
                || address.is_multicast()
                || address.segments()[0..2] == [0x2001, 0x0db8]
        }
    }
}

#[async_trait]
impl PublicSource for SearxngSource {
    fn name(&self) -> &'static str {
        "SearXNG"
    }
    fn supports(&self, _: &ResearchPlan) -> bool {
        true
    }

    fn disclosures(&self, _: &ResearchPlan) -> Vec<ConnectorDisclosure> {
        let destination = Url::parse(&self.endpoint)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_else(|| "configured SearXNG endpoint".into());
        vec![ConnectorDisclosure {
            id: "searxng".into(),
            service: "Configured SearXNG".into(),
            destinations: vec![destination],
            outbound_data: "the minimized public research query".into(),
            purpose: "search the configured metasearch instance".into(),
            risk: ConnectorRisk::PublicQuery,
            automatic_eligible: true,
        }]
    }

    async fn search(&self, plan: &ResearchPlan, limit: usize) -> Result<SourceOutput> {
        let mut request_url = Url::parse(&format!("{}/search", self.endpoint))?;
        request_url
            .query_pairs_mut()
            .append_pair("q", &plan.query)
            .append_pair("format", "json");
        let response = self
            .client
            .get(request_url.clone())
            .send()
            .await?
            .error_for_status()?;
        let response: Value = json_limited(response, 4_000_000, "SearXNG").await?;
        let results = response
            .get("results")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                anyhow!(
                    "SearXNG JSON output is unavailable; enable the json format on the instance"
                )
            })?;
        let now = Utc::now();
        let mut findings = Vec::new();
        let mut sources = Vec::new();
        for row in results.iter().take(limit) {
            let title = row
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Untitled result");
            let url = row.get("url").and_then(Value::as_str).unwrap_or("");
            if !matches!(
                url::Url::parse(url)
                    .ok()
                    .map(|u| u.scheme().to_owned())
                    .as_deref(),
                Some("http" | "https")
            ) {
                continue;
            }
            let body = row
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("Search result; inspect the source before use.");
            if !query_record_relevant(&plan.query, &format!("{title} {body}")) {
                continue;
            }
            let id = stable_id("searxng", url);
            findings.push(Finding {
                id: stable_id("finding", &format!("{id}:{title}")),
                title: title.into(),
                body: body.into(),
                facet: best_facet(plan),
                confidence: Confidence::Low,
                source_ids: vec![id.clone()],
                content_trust: ContentTrust::ExternalUntrusted,
                tags: vec!["discovery".into()],
            });
            sources.push(SourceRecord {
                id,
                title: title.into(),
                url: url.into(),
                publisher: url::Url::parse(url)
                    .ok()
                    .and_then(|u| u.host_str().map(str::to_owned))
                    .unwrap_or_else(|| "Unknown website".into()),
                retrieved_at: now,
                published_at: None,
                license: None,
                source_type: SourceType::SearchIndex,
                quality: SourceQuality::DiscoveryOnly,
                content_hash: Some(hash_text(body)),
                provenance: ProvenanceDetails {
                    request_url: Some(request_url.to_string()),
                    ..Default::default()
                },
            });
        }
        Ok(SourceOutput {
            connector: self.name().into(),
            findings,
            metrics: Vec::new(),
            sources,
            warnings: Vec::new(),
            audit: ConnectorAudit::default(),
        })
    }
}

pub fn source_catalog_findings(plan: &ResearchPlan) -> SourceOutput {
    let mut entries: Vec<(&str, &str, &str, Facet, SourceType)> = Vec::new();
    let lower = plan.query.to_lowercase();
    let us_context = is_us_context(&lower);
    if plan
        .facets
        .iter()
        .any(|f| matches!(f, Facet::Health | Facet::Transmission))
    {
        entries.push((
            "WHO Global Health Observatory",
            "Official global health indicators and country data.",
            "https://www.who.int/data/gho",
            Facet::Health,
            SourceType::Intergovernmental,
        ));
        if us_context {
            entries.push((
                "CDC Data & Statistics",
                "Official United States public-health datasets and surveillance portals.",
                "https://www.cdc.gov/datastatistics/",
                Facet::Health,
                SourceType::Government,
            ));
        }
    }
    if plan.facets.contains(&Facet::Safety) {
        entries.push((
            "UNDRR Datahub",
            "Global disaster-risk datasets and hazard definitions; dataset coverage and methodology still require inspection.",
            "https://data.undrr.org/",
            Facet::Safety,
            SourceType::Intergovernmental,
        ));
        if us_context {
            entries.push(("FBI Crime Data Explorer", "Official United States crime data; definitions and voluntary reporting coverage must be checked.", "https://cde.ucr.cjis.gov/", Facet::Safety, SourceType::Government));
            entries.push((
                "FEMA OpenFEMA",
                "Official United States disaster, emergency, and program datasets.",
                "https://www.fema.gov/about/openfema",
                Facet::Safety,
                SourceType::Government,
            ));
        }
    }
    if plan.facets.contains(&Facet::Textbooks) {
        entries.push((
            "OpenStax",
            "Peer-reviewed, openly licensed textbooks with subject-specific reuse terms.",
            "https://openstax.org/subjects",
            Facet::Textbooks,
            SourceType::OpenEducation,
        ));
        entries.push((
            "Open Textbook Library",
            "Catalog of openly licensed textbooks with faculty reviews.",
            "https://open.umn.edu/opentextbooks",
            Facet::Textbooks,
            SourceType::OpenEducation,
        ));
    }
    if plan.facets.contains(&Facet::Law) && us_context {
        entries.push((
            "GovInfo",
            "Official United States laws, regulations, bills, court opinions, and government publications; verify jurisdiction and effective date.",
            "https://www.govinfo.gov/",
            Facet::Law,
            SourceType::Government,
        ));
    }
    if plan.facets.contains(&Facet::Engineering) {
        entries.push((
            "NIST Standards.gov",
            "United States standards policy and conformity-assessment entry point; standards documents may have separate copyright and access terms.",
            "https://www.nist.gov/standardsgov",
            Facet::Engineering,
            SourceType::Government,
        ));
        entries.push((
            "NIST Guide to the SI",
            "Authoritative SI usage, conversion, and measurement guidance; retain units, uncertainty, and revision date.",
            "https://www.nist.gov/pml/special-publication-811",
            Facet::Engineering,
            SourceType::Government,
        ));
    }
    if plan.facets.contains(&Facet::Science)
        && query_has_any_word(
            &plan.query,
            &[
                "chemical",
                "chemistry",
                "compound",
                "molecule",
                "molecular",
                "element",
                "spectra",
            ],
        )
    {
        entries.push((
            "PubChem",
            "NIH chemical structures, formulas, properties, safety annotations, and literature links; records aggregate multiple depositors and must be traced.",
            "https://pubchem.ncbi.nlm.nih.gov/",
            Facet::Science,
            SourceType::Government,
        ));
        entries.push((
            "NIST Chemistry WebBook",
            "Reference thermochemical, spectral, and physical-property data with dataset-specific conditions and uncertainty.",
            "https://webbook.nist.gov/chemistry/",
            Facet::Science,
            SourceType::Government,
        ));
    }
    if plan.facets.contains(&Facet::Psychology) {
        entries.push((
            "PubMed",
            "NIH literature index for psychology, psychiatry, medicine, and behavioral science; metadata is discovery evidence, not proof of a conclusion.",
            "https://pubmed.ncbi.nlm.nih.gov/",
            Facet::Psychology,
            SourceType::SearchIndex,
        ));
    }
    if plan.facets.contains(&Facet::Assets) && is_3d_asset_request(&plan.query) {
        entries.push((
            "NIH 3D",
            "Government-operated biomedical 3D model and print-file repository; inspect each model's validation, intended use, and license before printing or clinical use.",
            "https://3d.nih.gov/",
            Facet::Assets,
            SourceType::Government,
        ));
        entries.push((
            "NASA 3D Resources",
            "NASA models, textures, and visualizations; individual asset rights and print suitability must be checked.",
            "https://nasa3d.arc.nasa.gov/",
            Facet::Assets,
            SourceType::Government,
        ));
    }
    if plan.facets.contains(&Facet::News) {
        entries.push((
            "GDELT Project",
            "Open global news-event discovery data; machine-coded events can be noisy and every material claim must be verified against the original publisher.",
            "https://www.gdeltproject.org/",
            Facet::News,
            SourceType::SearchIndex,
        ));
    }
    let now = Utc::now();
    let mut findings = Vec::new();
    let mut sources = Vec::new();
    for (title, description, url, facet, source_type) in entries {
        let id = stable_id("catalog", url);
        findings.push(Finding {
            id: stable_id("finding", &format!("{id}:{title}")),
            title: title.into(),
            body: format!("{description} This is a curated entry point; no underlying dataset was ingested in this run."),
            facet,
            confidence: Confidence::Low,
            source_ids: vec![id.clone()],
            content_trust: ContentTrust::CuratedTemplate,
            tags: vec!["official-entry-point".into()],
        });
        sources.push(SourceRecord {
            id,
            title: title.into(),
            url: url.into(),
            publisher: title.into(),
            retrieved_at: now,
            published_at: None,
            license: None,
            source_type,
            quality: SourceQuality::DiscoveryOnly,
            content_hash: None,
            provenance: ProvenanceDetails::default(),
        });
    }
    if plan.facets.contains(&Facet::Textbooks)
        && contains_phrase(&plan.query, "integration by parts")
    {
        let url = "https://openstax.org/books/calculus-volume-2/pages/3-1-integration-by-parts";
        let id = stable_id("openstax-section", url);
        findings.push(Finding {
            id: stable_id("finding", &format!("{id}:integration-by-parts")),
            title: "OpenStax Calculus Volume 2 — 3.1 Integration by Parts".into(),
            body: "Curated discovery link to the official OpenStax Calculus Volume 2 section on integration by parts. Inquiry did not retrieve or validate the page in this run. Open it to verify the current section, revision, authors, accessibility formats, errata, license, and reuse restrictions.".into(),
            facet: Facet::Textbooks,
            confidence: Confidence::Low,
            source_ids: vec![id.clone()],
            content_trust: ContentTrust::CuratedTemplate,
            tags: vec!["curated-open-textbook-link".into(), "official-openstax".into()],
        });
        let registry_reviewed_at = chrono::DateTime::parse_from_rfc3339("2026-07-16T00:00:00Z")
            .expect("valid registry review timestamp")
            .with_timezone(&Utc);
        sources.push(SourceRecord {
            id,
            title: "OpenStax Calculus Volume 2, section 3.1: Integration by Parts".into(),
            url: url.into(),
            publisher: "OpenStax, Rice University".into(),
            retrieved_at: registry_reviewed_at,
            published_at: None,
            license: None,
            source_type: SourceType::OpenEducation,
            quality: SourceQuality::DiscoveryOnly,
            content_hash: None,
            provenance: ProvenanceDetails {
                dataset_id: Some("OpenStax Calculus Volume 2, section 3.1".into()),
                request_url: None,
                methodology_url: None,
                source_updated_at: Some("curated registry reviewed 2026-07-16".into()),
                ..Default::default()
            },
        });
    }
    SourceOutput {
        connector: "Curated public-source catalog".into(),
        findings,
        metrics: Vec::new(),
        sources,
        warnings: Vec::new(),
        audit: ConnectorAudit::default(),
    }
}

fn is_us_context(query: &str) -> bool {
    let padded = format!(" {query} ");
    [
        " united states ",
        " u.s. ",
        " u.s.a. ",
        " usa ",
        " american ",
    ]
    .iter()
    .any(|needle| padded.contains(needle))
}

pub fn deduplicate(outputs: Vec<SourceOutput>) -> SourceOutput {
    let mut findings = BTreeMap::new();
    let mut metrics = Vec::new();
    let mut sources = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut audit = ConnectorAudit::default();
    let connector = outputs
        .iter()
        .map(|o| o.connector.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    for output in outputs {
        warnings.extend(output.warnings);
        audit.attempted.extend(output.audit.attempted);
        audit.succeeded.extend(output.audit.succeeded);
        audit.errors.extend(output.audit.errors);
        for finding in output.findings {
            findings.entry(finding.id.clone()).or_insert(finding);
        }
        metrics.extend(output.metrics);
        for source in output.sources {
            sources.entry(source.id.clone()).or_insert(source);
        }
    }
    metrics.sort_by(|left, right| {
        left.label
            .cmp(&right.label)
            .then_with(|| left.period.cmp(&right.period))
            .then_with(|| left.unit.cmp(&right.unit))
            .then_with(|| left.source_ids.cmp(&right.source_ids))
    });
    let mut findings = findings.into_values().collect::<Vec<_>>();
    findings.sort_by(|left, right| {
        confidence_rank(left.confidence)
            .cmp(&confidence_rank(right.confidence))
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
            .then_with(|| left.id.cmp(&right.id))
    });
    SourceOutput {
        connector,
        findings,
        metrics,
        sources: sources.into_values().collect(),
        warnings,
        audit,
    }
}

const fn confidence_rank(confidence: Confidence) -> u8 {
    match confidence {
        Confidence::High => 0,
        Confidence::Moderate => 1,
        Confidence::Low => 2,
    }
}

fn best_facet(plan: &ResearchPlan) -> Facet {
    let mut candidates = plan
        .facets
        .iter()
        .copied()
        .filter(|facet| *facet != Facet::Overview);
    let first = candidates.next();
    if candidates.next().is_none() {
        first.unwrap_or(Facet::Overview)
    } else {
        Facet::Overview
    }
}
fn stable_id(prefix: &str, value: &str) -> String {
    format!("{}-{}", prefix, &hash_text(value)[..12])
}
fn safe_http_url(value: &str) -> Option<&str> {
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then_some(value)
}
fn strict_media_https_url(value: Option<&str>, allowed_hosts: &[&str]) -> Result<String> {
    let value = value.ok_or_else(|| anyhow!("media metadata omitted a required URL"))?;
    let parsed = Url::parse(value).context("media metadata supplied an invalid URL")?;
    if parsed.scheme() != "https"
        || !parsed
            .host_str()
            .is_some_and(|host| allowed_hosts.contains(&host))
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
    {
        bail!("media URL did not pass the HTTPS host allowlist");
    }
    Ok(value.to_owned())
}
fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let mut shortened = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    shortened.push('…');
    shortened
}
fn strip_markup(value: &str) -> String {
    let tags = Regex::new(r"<[^>]*>").expect("valid markup regex");
    let highlights = Regex::new(r#"(?i)(?:<\s*)?/?span(?:\s+class="qt\d+")?\s*>?"#)
        .expect("valid MedlinePlus highlight regex");
    decode_basic_entities(&highlights.replace_all(&tags.replace_all(value, " "), " "))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn decode_basic_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}
fn clean_openalex_query(value: &str) -> String {
    let ignored = [
        "find",
        "recent",
        "current",
        "psychology",
        "psychological",
        "paper",
        "papers",
        "study",
        "studies",
        "journal",
        "literature",
        "scholarly",
        "peer",
        "reviewed",
        "statistics",
        "statistical",
        "effect",
        "effects",
        "size",
        "sizes",
        "on",
        "with",
        "about",
        "the",
        "a",
        "an",
    ];
    let cleaned = normalize_for_matching(value)
        .split_whitespace()
        .filter(|term| !ignored.contains(term))
        .take(10)
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        normalize_for_matching(value)
    } else {
        cleaned
    }
}

fn is_scholarly_request(query: &str) -> bool {
    query_has_any_word(
        query,
        &[
            "paper",
            "papers",
            "study",
            "studies",
            "journal",
            "literature",
            "scholarly",
        ],
    )
}

fn health_subject_query(value: &str) -> String {
    let ignored = [
        "find",
        "explain",
        "symptom",
        "symptoms",
        "sign",
        "signs",
        "transmission",
        "route",
        "routes",
        "spread",
        "prevention",
        "prevent",
        "treatment",
        "disease",
        "health",
        "medical",
        "and",
        "or",
        "of",
        "the",
        "a",
        "an",
    ];
    let cleaned = normalize_for_matching(value)
        .split_whitespace()
        .filter(|term| !ignored.contains(term))
        .take(5)
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        normalize_for_matching(value)
    } else {
        cleaned
    }
}
fn wikipedia_search_query(value: &str) -> String {
    if let Some(office) = current_office_search_phrase(value) {
        return office;
    }
    if matches!(resolve_intent(value).kind, IntentKind::RecentEventMedia) {
        return recent_event_search_query(value);
    }
    value.to_owned()
}

fn recent_event_search_query(value: &str) -> String {
    let terms = gdelt_event_query(value).unwrap_or_else(|_| normalize_for_matching(value));
    let has_explicit_year = normalize_for_matching(value)
        .split_whitespace()
        .any(|token| {
            token.len() == 4
                && token
                    .parse::<i32>()
                    .is_ok_and(|year| (1900..=2100).contains(&year))
        });
    if has_explicit_year {
        terms
    } else {
        format!("{} {terms}", Utc::now().year())
    }
}

/// Resolve common officeholder wording without asking a language model to name
/// a person. A jurisdiction is mandatory: a bare "current president" is
/// intentionally left unresolved instead of silently assuming the United
/// States or accepting generic encyclopedia results.
fn current_office_search_phrase(value: &str) -> Option<String> {
    let normalized = normalize_for_matching(value);
    if normalized.split_whitespace().any(|token| token == "potus") {
        return Some("President of United States".into());
    }
    let mut tokens = normalized.split_whitespace().collect::<Vec<_>>();
    if tokens.is_empty() {
        return None;
    }

    let (office_start, office_len, office_title) = tokens
        .windows(2)
        .position(|pair| pair == ["prime", "minister"])
        .map(|index| (index, 2, "Prime Minister"))
        .or_else(|| {
            tokens
                .iter()
                .position(|token| *token == "president")
                .map(|index| (index, 1, "President"))
        })
        .or_else(|| {
            tokens
                .iter()
                .position(|token| *token == "chancellor")
                .map(|index| (index, 1, "Chancellor"))
        })
        .or_else(|| {
            tokens
                .iter()
                .position(|token| matches!(*token, "king" | "monarch"))
                .map(|index| (index, 1, "Monarch"))
        })?;

    let suffix_chatter = [
        "photo",
        "photos",
        "image",
        "images",
        "picture",
        "pictures",
        "portrait",
        "portraits",
        "bio",
        "biography",
        "bios",
        "source",
        "sources",
        "include",
        "including",
        "provide",
        "show",
        "showing",
        "with",
        "timeline",
        "explorer",
        "number",
    ];
    let leading_chatter = [
        "who", "what", "which", "is", "was", "are", "the", "a", "an", "current", "now", "right",
        "please", "find", "show", "tell", "me", "photo", "image", "picture", "portrait", "of",
        "number",
    ];
    let is_ordinal = |token: &str| {
        let digits = token.trim_end_matches(|character: char| character.is_ascii_alphabetic());
        !digits.is_empty() && digits.chars().all(|character| character.is_ascii_digit())
    };
    let clean_candidate = |candidate: &mut Vec<&str>| {
        while candidate
            .first()
            .is_some_and(|token| leading_chatter.contains(token) || is_ordinal(token))
        {
            candidate.remove(0);
        }
        if let Some(index) = candidate
            .iter()
            .position(|token| suffix_chatter.contains(token))
        {
            candidate.truncate(index);
        }
        while candidate
            .last()
            .is_some_and(|token| suffix_chatter.contains(token) || *token == "of")
        {
            candidate.pop();
        }
    };

    let mut before = tokens.drain(..office_start).collect::<Vec<_>>();
    let mut after = tokens.drain(office_len..).collect::<Vec<_>>();
    let explicitly_of = after.first() == Some(&"of");
    if explicitly_of {
        after.remove(0);
    }
    clean_candidate(&mut before);
    clean_candidate(&mut after);
    let mut place = if explicitly_of || before.is_empty() {
        after
    } else {
        before
    };
    clean_candidate(&mut place);
    if place.is_empty() {
        return None;
    }

    let place = canonical_office_jurisdiction(&place.join(" "))?;
    if office_title == "Monarch" && place == "United Kingdom" {
        Some("Monarch of the United Kingdom".into())
    } else {
        Some(format!("{office_title} of {place}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OfficialCurrentOffice {
    UnitedStatesPresident,
    UnitedKingdomMonarch,
}

pub(crate) fn official_current_office(value: &str) -> Option<OfficialCurrentOffice> {
    if query_office_ordinal(value).is_some() {
        return None;
    }
    let office = current_office_search_phrase(value)?;
    if office_labels_match(&office, "President of the United States") {
        Some(OfficialCurrentOffice::UnitedStatesPresident)
    } else if office_labels_match(&office, "Monarch of the United Kingdom") {
        Some(OfficialCurrentOffice::UnitedKingdomMonarch)
    } else {
        None
    }
}

pub fn office_query_needs_jurisdiction(value: &str) -> bool {
    let normalized = normalize_for_matching(value);
    let mentions_office = [
        "president",
        "prime minister",
        "chancellor",
        "king",
        "monarch",
    ]
    .iter()
    .any(|office| contains_phrase(&normalized, office));
    let asks_for_identity = ["current", "currently", "right now", "today"]
        .iter()
        .any(|term| contains_phrase(&normalized, term))
        || query_office_ordinal(value).is_some();
    mentions_office && asks_for_identity && current_office_search_phrase(value).is_none()
}

fn canonical_office_jurisdiction(value: &str) -> Option<String> {
    let normalized = normalize_for_matching(value);
    let normalized = normalized.strip_prefix("the ").unwrap_or(&normalized);
    let us_alias = matches!(
        normalized,
        "us" | "u s"
            | "usa"
            | "u s a"
            | "united states"
            | "united states of america"
            | "america"
            | "american"
    );
    if us_alias {
        return Some("United States".into());
    }
    let uk_alias = matches!(
        normalized,
        "uk" | "u k" | "united kingdom" | "britain" | "great britain" | "british"
    );
    if uk_alias {
        return Some("United Kingdom".into());
    }
    let invalid = [
        "right",
        "right now",
        "now",
        "today",
        "currently",
        "world",
        "country",
        "photo",
        "image",
        "biography",
        "source",
        "sources",
    ];
    if normalized.is_empty()
        || invalid.contains(&normalized)
        || !normalized
            .chars()
            .any(|character| character.is_alphabetic())
    {
        return None;
    }
    Some(
        normalized
            .split_whitespace()
            .map(|word| {
                let mut characters = word.chars();
                characters
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}
fn query_office_ordinal(value: &str) -> Option<String> {
    Regex::new(
        r"(?ix)
        (?:
          \b(\d+)(?:st|nd|rd|th)?\s+(?:(?:u\.?s\.?|u\.?s\.?a\.?)\s+)?(?:president|potus|prime\s+minister|chancellor)\b
          |
          \b(?:president|potus|prime\s+minister|chancellor)\s*(?:\#|number\s+)?\s*(\d+)\b
          |
          \b(?:(?:u\.?s\.?|u\.?s\.?a\.?)\s+)?(?:president|potus)\s+number\s+(\d+)\b
        )",
    )
        .expect("valid ordinal office regex")
        .captures(value)
        .and_then(|captures| {
            captures
                .get(1)
                .or_else(|| captures.get(2))
                .or_else(|| captures.get(3))
        })
        .and_then(|value| {
            value
                .as_str()
                .parse::<u16>()
                .ok()
                .filter(|ordinal| (1..=999).contains(ordinal))
                .map(|ordinal| ordinal.to_string())
        })
}
fn office_labels_match(left: &str, right: &str) -> bool {
    let canonical = |value: &str| {
        normalize_for_matching(value)
            .split_whitespace()
            .filter(|word| *word != "the")
            .collect::<Vec<_>>()
            .join(" ")
    };
    canonical(left) == canonical(right)
}

fn officeholder_display_name(office_phrase: &str, person_name: &str, person: &Value) -> String {
    if !office_labels_match(office_phrase, "Monarch of the United Kingdom") {
        return person_name.into();
    }
    let title = person
        .pointer("/claims/P21")
        .and_then(Value::as_array)
        .and_then(|statements| {
            statements
                .iter()
                .filter(|statement| {
                    statement.get("rank").and_then(Value::as_str) != Some("deprecated")
                })
                .find_map(|statement| {
                    match statement
                        .pointer("/mainsnak/datavalue/value/id")
                        .and_then(Value::as_str)
                    {
                        Some("Q6581097") => Some("King"),
                        Some("Q6581072") => Some("Queen"),
                        _ => None,
                    }
                })
        });
    match title {
        Some(title) if !contains_phrase(person_name, title) => format!("{title} {person_name}"),
        _ => person_name.into(),
    }
}

fn portrait_metadata_identifies_person(
    filename: &str,
    description: &str,
    person_name: &str,
) -> bool {
    let identity_text = normalize_for_matching(&format!("{filename} {description}"));
    let name_tokens = person_name
        .split_whitespace()
        .map(normalize_for_matching)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    !name_tokens.is_empty()
        && name_tokens
            .iter()
            .all(|token| contains_phrase(&identity_text, token))
}

fn wikidata_office_result_matches(result: &Value, office_phrase: &str) -> bool {
    let label = result.get("label").and_then(Value::as_str).unwrap_or("");
    let description = normalize_for_matching(
        result
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    let recognized_description = contains_phrase(&description, "head of state")
        || contains_phrase(&description, "head of government")
        || (office_labels_match(office_phrase, "Monarch of the United Kingdom")
            && contains_phrase(&description, "king or queen"));
    office_labels_match(label, office_phrase) && recognized_description
}
fn qualifier_time(statement: &Value, property: &str) -> Option<String> {
    statement
        .pointer(&format!("/qualifiers/{property}/0/datavalue/value/time"))
        .and_then(Value::as_str)
        .and_then(|value| value.trim_start_matches('+').get(..10))
        .map(str::to_owned)
}

fn qualifier_date(statement: &Value, property: &str) -> Result<Option<chrono::NaiveDate>> {
    let qualifier = statement.pointer(&format!("/qualifiers/{property}/0"));
    let Some(qualifier) = qualifier else {
        return Ok(None);
    };
    let raw = qualifier
        .pointer("/datavalue/value/time")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Wikidata supplied malformed {property} officeholder qualifier"))?;
    let date = raw
        .trim_start_matches('+')
        .get(..10)
        .and_then(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .ok_or_else(|| anyhow!("Wikidata supplied invalid {property} officeholder date"))?;
    Ok(Some(date))
}

fn select_current_officeholder_statement(statements: &[Value]) -> Result<Option<&Value>> {
    let today = Utc::now().date_naive();
    let mut current = Vec::new();
    for statement in statements {
        if statement.get("rank").and_then(Value::as_str) == Some("deprecated") {
            continue;
        }
        let has_started = qualifier_date(statement, "P580")?.is_none_or(|start| start <= today);
        let has_not_ended = qualifier_date(statement, "P582")?.is_none_or(|end| end >= today);
        if has_started && has_not_ended {
            current.push(statement);
        }
    }
    let distinct_people = current
        .iter()
        .filter_map(|statement| {
            statement
                .pointer("/mainsnak/datavalue/value/id")
                .and_then(Value::as_str)
        })
        .collect::<BTreeSet<_>>();
    if distinct_people.len() > 1 {
        bail!(
            "Wikidata has {} distinct unexpired officeholders; Inquiry abstained instead of selecting one",
            distinct_people.len()
        );
    }
    Ok(current
        .iter()
        .copied()
        .find(|statement| statement.get("rank").and_then(Value::as_str) == Some("preferred"))
        .or_else(|| current.first().copied()))
}
fn ordinal_label(value: &str) -> String {
    let number = value.parse::<u64>().ok();
    let suffix = match number.map(|number| number % 100) {
        Some(11..=13) => "th",
        _ => match number.map(|number| number % 10) {
            Some(1) => "st",
            Some(2) => "nd",
            Some(3) => "rd",
            _ => "th",
        },
    };
    format!("{value}{suffix}")
}
fn normalize_for_matching(value: &str) -> String {
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
fn contains_phrase(value: &str, phrase: &str) -> bool {
    let normalized = normalize_for_matching(value);
    let phrase = normalize_for_matching(phrase);
    format!(" {normalized} ").contains(&format!(" {phrase} "))
}
fn is_image_request(query: &str) -> bool {
    query_has_any_word(
        query,
        &[
            "image",
            "images",
            "photo",
            "photos",
            "picture",
            "portrait",
            "diagram",
            "illustration",
            "anatomy image",
            "anatomy photo",
        ],
    )
}
fn is_3d_asset_request(query: &str) -> bool {
    query_has_any_word(
        query,
        &[
            "3d model",
            "3d printable",
            "3d print",
            "stl",
            "obj file",
            "glb",
            "gltf",
            "mesh",
            "print file",
        ],
    )
}
fn commons_search_query(query: &str) -> String {
    if let Some(office) = current_office_search_phrase(query) {
        return office;
    }
    if (contains_phrase(query, "anatomy") || contains_phrase(query, "anatomical"))
        && contains_phrase(query, "heart")
    {
        // Treat requested labels and viewpoints as verification criteria, not
        // mandatory search terms. Commons search becomes brittle when every
        // desired feature is required, while the returned record still tells
        // the caller to inspect those features before use.
        return "human heart anatomy diagram".into();
    }
    let ignored = [
        "find",
        "show",
        "licensed",
        "license",
        "image",
        "images",
        "photo",
        "photos",
        "picture",
        "pictures",
        "showing",
        "include",
        "provide",
        "open",
        "source",
        "official",
        "major",
        "great",
        "anatomy",
        "anatomical",
        "and",
        "of",
        "the",
        "a",
        "an",
        "for",
        "me",
        "recent",
        "latest",
        "current",
        "today",
        "yesterday",
    ];
    let mut terms = normalize_for_matching(query)
        .split_whitespace()
        .filter(|term| !ignored.contains(term))
        .take(10)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if contains_phrase(query, "anatomy") || contains_phrase(query, "anatomical") {
        terms.push("diagram".into());
    }
    if matches!(resolve_intent(query).kind, IntentKind::RecentEventMedia) {
        return recent_event_search_query(query);
    }
    if terms.is_empty() {
        query.trim().to_owned()
    } else {
        terms.join(" ")
    }
}
fn query_record_relevant(query: &str, record: &str) -> bool {
    let ignored = [
        "find",
        "show",
        "include",
        "with",
        "licensed",
        "license",
        "free",
        "open",
        "source",
        "current",
        "official",
        "image",
        "images",
        "photo",
        "photos",
        "picture",
        "diagram",
        "illustration",
        "human",
        "showing",
        "major",
        "provide",
        "sourced",
        "biography",
        "and",
        "paper",
        "papers",
        "study",
        "studies",
        "journal",
        "literature",
        "scholarly",
        "recent",
        "peer",
        "reviewed",
        "symptom",
        "symptoms",
        "transmission",
        "route",
        "routes",
        "what",
        "who",
        "where",
        "when",
        "why",
        "how",
        "is",
        "are",
        "was",
        "were",
        "does",
        "do",
        "the",
        "of",
        "for",
        "from",
        "on",
        "in",
        "to",
        "a",
        "an",
        "me",
    ];
    let normalized_query = if matches!(resolve_intent(query).kind, IntentKind::RecentEventMedia) {
        recent_event_search_query(query)
    } else {
        normalize_for_matching(query)
    };
    let query_tokens = normalized_query
        .split_whitespace()
        .filter(|token| token.len() >= 3 && !ignored.contains(token))
        .collect::<BTreeSet<_>>();
    if query_tokens.is_empty() {
        return false;
    }
    let normalized_record = normalize_for_matching(record);
    let record_tokens = normalized_record
        .split_whitespace()
        .collect::<BTreeSet<_>>();
    let overlap = query_tokens.intersection(&record_tokens).count();
    overlap >= query_tokens.len().min(2)
}
fn event_match_score(query: &str, record: &str) -> usize {
    let query = recent_event_search_query(query);
    let query_tokens = normalize_for_matching(&query)
        .split_whitespace()
        .filter(|token| token.len() >= 2)
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let record_tokens = normalize_for_matching(record)
        .split_whitespace()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    query_tokens.intersection(&record_tokens).count()
}
fn media_record_relevant(query: &str, record: &str) -> bool {
    let ignored = [
        "find",
        "show",
        "include",
        "provide",
        "licensed",
        "license",
        "free",
        "open",
        "source",
        "current",
        "official",
        "image",
        "images",
        "photo",
        "photos",
        "picture",
        "pictures",
        "diagram",
        "illustration",
        "anatomy",
        "anatomical",
        "human",
        "showing",
        "major",
        "great",
        "chamber",
        "chambers",
        "vessel",
        "vessels",
        "labeled",
        "labelled",
        "labels",
        "structure",
        "structures",
        "view",
        "viewpoint",
        "and",
        "the",
        "of",
        "for",
        "a",
        "an",
        "me",
        "recent",
        "latest",
        "today",
        "yesterday",
    ];
    let normalized_query = if matches!(resolve_intent(query).kind, IntentKind::RecentEventMedia) {
        recent_event_search_query(query)
    } else {
        normalize_for_matching(query)
    };
    let query_tokens = normalized_query
        .split_whitespace()
        .filter(|token| token.len() >= 3 && !ignored.contains(token))
        .collect::<BTreeSet<_>>();
    if query_tokens.is_empty() {
        return false;
    }
    let normalized_record = normalize_for_matching(record);
    let record_tokens = normalized_record
        .split_whitespace()
        .collect::<BTreeSet<_>>();
    query_tokens.intersection(&record_tokens).count() >= query_tokens.len().min(2)
}
fn query_has_any_word(value: &str, words: &[&str]) -> bool {
    words.iter().any(|word| contains_phrase(value, word))
}
fn country_query_matches(query: &str, world_bank_name: &str) -> bool {
    if contains_phrase(query, world_bank_name) {
        return true;
    }
    let aliases: &[&str] = match world_bank_name {
        "Russian Federation" => &["russia"],
        "Korea, Rep." => &["south korea"],
        "Korea, Dem. People's Rep." => &["north korea"],
        "Egypt, Arab Rep." => &["egypt"],
        "Iran, Islamic Rep." => &["iran"],
        "Venezuela, RB" => &["venezuela"],
        "Yemen, Rep." => &["yemen"],
        "Congo, Dem. Rep." => &["democratic republic of the congo", "dr congo", "drc"],
        "Congo, Rep." => &["republic of the congo", "congo brazzaville"],
        "Gambia, The" => &["gambia"],
        "Bahamas, The" => &["bahamas"],
        "Slovak Republic" => &["slovakia"],
        "Kyrgyz Republic" => &["kyrgyzstan"],
        "Lao PDR" => &["laos"],
        "Syrian Arab Republic" => &["syria"],
        "Turkiye" | "Türkiye" => &["turkey"],
        "Viet Nam" => &["vietnam"],
        _ => &[],
    };
    aliases.iter().any(|alias| contains_phrase(query, alias))
}

fn reject_ambiguous_country_query(query: &str) -> Result<()> {
    if contains_phrase(query, "georgia")
        && ![
            "country of georgia",
            "georgia country",
            "georgia the country",
        ]
        .iter()
        .any(|qualifier| contains_phrase(query, qualifier))
    {
        bail!(
            "'Georgia' is ambiguous between a country and other jurisdictions; specify 'country of Georgia' or the intended state/region"
        );
    }
    if contains_phrase(query, "congo")
        && ![
            "democratic republic of the congo",
            "dr congo",
            "drc",
            "republic of the congo",
            "congo brazzaville",
        ]
        .iter()
        .any(|qualifier| contains_phrase(query, qualifier))
    {
        bail!(
            "'Congo' is ambiguous; specify Democratic Republic of the Congo or Republic of the Congo"
        );
    }
    if contains_phrase(query, "korea")
        && !["south korea", "north korea"]
            .iter()
            .any(|qualifier| contains_phrase(query, qualifier))
    {
        bail!("'Korea' is ambiguous; specify South Korea or North Korea");
    }
    Ok(())
}
fn human_number(value: f64) -> String {
    let abs = value.abs();
    if abs >= 1_000_000_000_000.0 {
        format!("{:.2}T", value / 1_000_000_000_000.0)
    } else if abs >= 1_000_000_000.0 {
        format!("{:.2}B", value / 1_000_000_000.0)
    } else if abs >= 1_000_000.0 {
        format!("{:.2}M", value / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.2}K", value / 1_000.0)
    } else {
        format!("{value:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(query: &str, facets: Vec<Facet>) -> ResearchPlan {
        ResearchPlan {
            query: query.into(),
            facets,
            terms: Vec::new(),
            rationale: String::new(),
        }
    }

    #[test]
    fn catalog_links_are_discovery_not_evidence() {
        let output =
            source_catalog_findings(&plan("dengue safety", vec![Facet::Health, Facet::Safety]));
        assert!(
            output
                .sources
                .iter()
                .all(|source| matches!(source.quality, SourceQuality::DiscoveryOnly))
        );
        assert!(
            output
                .findings
                .iter()
                .all(|finding| matches!(finding.confidence, Confidence::Low))
        );
    }

    #[test]
    fn us_only_sources_require_us_context() {
        let global = source_catalog_findings(&plan("safety in Kenya", vec![Facet::Safety]));
        assert!(
            !global
                .sources
                .iter()
                .any(|source| source.title.contains("FBI"))
        );
        assert!(
            !global
                .sources
                .iter()
                .any(|source| source.title.contains("FEMA"))
        );

        let us = source_catalog_findings(&plan("safety in the United States", vec![Facet::Safety]));
        assert!(us.sources.iter().any(|source| source.title.contains("FBI")));
        assert!(
            us.sources
                .iter()
                .any(|source| source.title.contains("FEMA"))
        );
    }

    #[test]
    fn openalex_search_removes_query_syntax_punctuation() {
        assert_eq!(
            clean_openalex_query("What are trends in ML? [review]*"),
            "what are trends in ml review"
        );
        assert_eq!(
            clean_openalex_query(
                "Recent psychology papers on retrieval practice with effect size statistics"
            ),
            "retrieval practice"
        );
        assert_eq!(
            health_subject_query("Dengue symptoms, transmission routes, and prevention"),
            "dengue"
        );
    }

    #[test]
    fn current_office_queries_are_rewritten_to_the_office_article() {
        assert_eq!(
            wikipedia_search_query(
                "Who is the current president of Kenya? Include a sourced biography"
            ),
            "President of Kenya"
        );
        assert_eq!(
            wikipedia_search_query("Who is the 47th president of the United States?"),
            "President of United States"
        );
        assert_eq!(
            current_office_search_phrase("Who is the current president of Kenya?").as_deref(),
            Some("President of Kenya")
        );
        assert_eq!(
            current_office_search_phrase("Current king of UK").as_deref(),
            Some("Monarch of the United Kingdom")
        );
        assert_eq!(
            current_office_search_phrase("Who is the UK king right now?").as_deref(),
            Some("Monarch of the United Kingdom")
        );
        assert_eq!(
            current_office_search_phrase("current British monarch").as_deref(),
            Some("Monarch of the United Kingdom")
        );
        for query in [
            "current us president",
            "current U.S. president",
            "current USA president",
            "US president",
            "president USA",
            "who is the US president right now",
            "current president of the United States photo",
            "photo of current president of America with sources",
            "47th US president",
            "who is POTUS",
            "current POTUS",
            "current American president",
            "US president number 46",
            "46 US president",
        ] {
            assert_eq!(
                current_office_search_phrase(query).as_deref(),
                Some("President of United States"),
                "failed to route {query:?}"
            );
        }
        assert_eq!(
            query_office_ordinal("47th US president").as_deref(),
            Some("47")
        );
        assert_eq!(query_office_ordinal("president #47").as_deref(), Some("47"));
        assert_eq!(
            query_office_ordinal("US president number 46").as_deref(),
            Some("46")
        );
        assert_eq!(
            query_office_ordinal("46 US president").as_deref(),
            Some("46")
        );
        assert_eq!(query_office_ordinal("president 47").as_deref(), Some("47"));
        assert_eq!(current_office_search_phrase("president 47"), None);
        assert_eq!(current_office_search_phrase("president number 47"), None);
        assert_eq!(current_office_search_phrase("current president"), None);
        assert_eq!(current_office_search_phrase("current king"), None);
        assert_eq!(current_office_search_phrase("current monarch"), None);
        assert!(office_query_needs_jurisdiction("current king"));
        assert!(office_query_needs_jurisdiction("current monarch"));
        assert_eq!(
            current_office_search_phrase("who is president right now"),
            None
        );
        assert_eq!(
            query_office_ordinal("Who was the 44th president of the United States?").as_deref(),
            Some("44")
        );
        assert!(office_labels_match(
            "President of the United States",
            "President of United States"
        ));
    }

    #[test]
    fn monarch_office_search_requires_an_exact_office_result() {
        let exact = serde_json::json!({
            "id": "Q9134365",
            "label": "Monarch of the United Kingdom",
            "description": "King or queen of the United Kingdom and its overseas territories since 1927"
        });
        let institution = serde_json::json!({
            "id": "Q739941",
            "label": "monarchy of the United Kingdom",
            "description": "constitutional monarchy of the United Kingdom"
        });
        let ship = serde_json::json!({
            "id": "Q56174473",
            "label": "British Monarch",
            "description": "ship"
        });

        assert!(wikidata_office_result_matches(
            &exact,
            "Monarch of the United Kingdom"
        ));
        assert!(!wikidata_office_result_matches(
            &institution,
            "Monarch of the United Kingdom"
        ));
        assert!(!wikidata_office_result_matches(
            &ship,
            "Monarch of the United Kingdom"
        ));
    }

    #[test]
    fn official_us_identity_checks_bind_name_and_link_to_the_same_section() {
        let valid = r#"
          <a href="https://www.whitehouse.gov/administration/melania-trump/">First Lady Melania Trump</a>
          <h3>Current president</h3>
          <p><a href="https://www.whitehouse.gov/administration/donald-j-trump/">The 47th and current president of the United States is Donald John Trump.</a></p>
          <h3>Past presidents</h3>
        "#;
        let section = usagov_current_president_section(valid).expect("current section");
        let text = normalize_for_matching(&strip_markup(section));
        assert!(person_name_tokens_match(&text, "Donald Trump"));
        assert!(!person_name_tokens_match(&text, "Melania Trump"));
        assert!(section.contains("/donald-j-trump/"));
        assert!(!section.contains("/melania-trump/"));

        let profile = r#"<meta property="og:title" content="President Donald J. Trump" />"#;
        let title = white_house_identity_title(profile).expect("identity title");
        assert!(person_name_tokens_match(&title, "Donald Trump"));
        assert!(!person_name_tokens_match(&title, "Melania Trump"));
    }

    #[test]
    fn official_uk_records_bind_current_reign_to_exact_wikidata_identity() {
        let reigns = "Start date,End date,Kingdom,Monarch\n\
            1952-02-06,2022-09-08,United Kingdom,Elizabeth II\n\
            2022-09-08,,United Kingdom,Charles III\n";
        let monarchs = "Title,Date of birth,Date of death,Wikidata ID\n\
            Elizabeth II,1926-04-21,2022-09-08,Q9682\n\
            Charles III,1948-11-14,,Q43274\n";

        let record = current_uk_monarch_from_parliament_csv(reigns, monarchs)
            .expect("one open UK reign should be accepted");
        assert_eq!(
            record,
            UkParliamentMonarchRecord {
                name: "Charles III".into(),
                reign_started: "2022-09-08".into(),
                birth_date: "1948-11-14".into(),
                wikidata_id: "Q43274".into(),
            }
        );

        let ambiguous = "Start date,End date,Kingdom,Monarch\n\
            2022-09-08,,United Kingdom,Charles III\n\
            2026-01-01,,United Kingdom,Example II\n";
        assert!(current_uk_monarch_from_parliament_csv(ambiguous, monarchs).is_err());
    }

    #[test]
    fn royal_family_biography_and_regnal_title_require_exact_identity() {
        let html = r#"<html><head><meta property="og:description" content="King Charles III, formerly known as The Prince of Wales, became King on the death of his mother Queen Elizabeth II on 8 September 2022." /></head></html>"#;
        assert!(royal_family_identity_summary(html, "Charles III").is_some());
        assert!(royal_family_identity_summary(html, "William Ruto").is_none());

        let male = serde_json::json!({
            "claims": {"P21": [{"rank": "normal", "mainsnak": {"datavalue": {"value": {"id": "Q6581097"}}}}]}
        });
        assert_eq!(
            officeholder_display_name("Monarch of the United Kingdom", "Charles III", &male),
            "King Charles III"
        );
        assert_eq!(
            officeholder_display_name("President of Kenya", "William Ruto", &male),
            "William Ruto"
        );
    }

    #[test]
    fn portrait_reuse_basis_must_be_explicit() {
        assert!(recognized_commons_license("Public domain", None));
        assert!(recognized_commons_license("CC0", None));
        assert!(recognized_commons_license(
            "CC BY-SA 4.0",
            Some("https://creativecommons.org/licenses/by-sa/4.0/")
        ));
        assert!(!recognized_commons_license("Attribution", None));
        assert!(!recognized_commons_license("CC BY-SA 4.0", None));
        assert!(!recognized_commons_license(
            "CC BY-SA 4.0",
            Some("https://example.com/licenses/by-sa/4.0/")
        ));
        assert!(!recognized_commons_license(
            "Creative Commons",
            Some("https://creativecommons.org/licenses/by-sa/4.0/")
        ));
        assert!(!recognized_commons_license(
            "CC BY 4.0",
            Some("https://creativecommons.org/licenses/by-sa/4.0/")
        ));
        assert!(!recognized_commons_license(
            "CC BY-SA 4.0",
            Some("https://creativecommons.org/licenses/by-sa/2.0/")
        ));
    }

    #[test]
    fn portrait_identity_matching_handles_regnal_names_without_weakening_binding() {
        assert!(portrait_metadata_identifies_person(
            "King Charles III (July 2023).jpg",
            "His Majesty at a bilateral meeting",
            "Charles III"
        ));
        assert!(portrait_metadata_identifies_person(
            "Official Presidential Portrait of President Donald J. Trump.jpg",
            "Donald Trump",
            "Donald Trump"
        ));
        assert!(!portrait_metadata_identifies_person(
            "King Charles III (July 2023).jpg",
            "His Majesty at a bilateral meeting",
            "William Ruto"
        ));
        assert!(!portrait_metadata_identifies_person(
            "King Charles II by John Riley.jpg",
            "Portrait of King Charles II",
            "King Charles III"
        ));
    }

    #[test]
    fn commons_queries_remove_request_chatter_but_keep_subject_features() {
        assert_eq!(
            commons_search_query(
                "Find a licensed anatomy image of the human heart showing chambers and major vessels"
            ),
            "human heart anatomy diagram"
        );
        assert!(media_record_relevant(
            "labeled anatomy image of the human heart chambers and great vessels",
            "Diagram of the human heart"
        ));
        let current_year = Utc::now().year().to_string();
        assert_eq!(
            wikipedia_search_query("show me a picture of the recent US bombing"),
            format!("{current_year} us bombing")
        );
        assert_eq!(
            commons_search_query("show me a picture of the recent US bombing"),
            format!("{current_year} us bombing")
        );
        assert!(query_record_relevant(
            "show me a picture of the recent US bombing",
            &format!("{current_year} Iran war and US bombing campaign")
        ));
    }

    #[test]
    fn current_officeholder_selection_ignores_expired_statements() {
        let statements = serde_json::json!([
            {"rank":"normal","qualifiers":{"P582":[{"datavalue":{"value":{"time":"+2020-01-01T00:00:00Z"}}}]}},
            {"rank":"preferred","mainsnak":{"datavalue":{"value":{"id":"Q-current"}}},"qualifiers":{"P580":[{"datavalue":{"value":{"time":"+2025-01-20T00:00:00Z"}}}]}}
        ]);
        let selected = select_current_officeholder_statement(statements.as_array().unwrap())
            .expect("unambiguous selection")
            .expect("current statement");
        assert_eq!(
            selected
                .pointer("/mainsnak/datavalue/value/id")
                .and_then(Value::as_str),
            Some("Q-current")
        );
        assert_eq!(
            qualifier_time(selected, "P580").as_deref(),
            Some("2025-01-20")
        );
    }

    #[test]
    fn current_officeholder_selection_abstains_on_distinct_people() {
        let statements = serde_json::json!([
            {"rank":"preferred","mainsnak":{"datavalue":{"value":{"id":"Q-one"}}}},
            {"rank":"normal","mainsnak":{"datavalue":{"value":{"id":"Q-two"}}}},
            {"rank":"deprecated","mainsnak":{"datavalue":{"value":{"id":"Q-old"}}}}
        ]);
        let error = select_current_officeholder_statement(statements.as_array().unwrap())
            .expect_err("distinct current people must be ambiguous");
        assert!(error.to_string().contains("2 distinct"));
    }

    #[test]
    fn strict_media_urls_require_https_and_an_allowlisted_host() {
        assert_eq!(
            strict_media_https_url(
                Some("https://upload.wikimedia.org/example.jpg"),
                &["upload.wikimedia.org"]
            )
            .expect("allowlisted HTTPS URL"),
            "https://upload.wikimedia.org/example.jpg"
        );
        assert!(
            strict_media_https_url(
                Some("http://upload.wikimedia.org/x.jpg"),
                &["upload.wikimedia.org"]
            )
            .is_err()
        );
        assert!(
            strict_media_https_url(Some("https://example.com/x.jpg"), &["upload.wikimedia.org"])
                .is_err()
        );
        assert!(
            strict_media_https_url(
                Some("https://upload.wikimedia.org:444/x.jpg"),
                &["upload.wikimedia.org"]
            )
            .is_err()
        );
    }

    #[test]
    fn current_officeholder_selection_ignores_future_statements() {
        let future = (Utc::now().date_naive() + chrono::Duration::days(30))
            .format("+%Y-%m-%dT00:00:00Z")
            .to_string();
        let current = (Utc::now().date_naive() - chrono::Duration::days(30))
            .format("+%Y-%m-%dT00:00:00Z")
            .to_string();
        let statements = serde_json::json!([
            {"rank":"preferred","mainsnak":{"datavalue":{"value":{"id":"Q-future"}}},"qualifiers":{"P580":[{"datavalue":{"value":{"time":future}}}]}},
            {"rank":"normal","mainsnak":{"datavalue":{"value":{"id":"Q-current"}}},"qualifiers":{"P580":[{"datavalue":{"value":{"time":current}}}]}}
        ]);
        let selected = select_current_officeholder_statement(statements.as_array().unwrap())
            .expect("unambiguous selection")
            .expect("current statement");
        assert_eq!(
            selected
                .pointer("/mainsnak/datavalue/value/id")
                .and_then(Value::as_str),
            Some("Q-current")
        );
    }

    #[test]
    fn current_officeholder_selection_rejects_malformed_supplied_dates() {
        let statements = serde_json::json!([{
            "rank":"preferred",
            "mainsnak":{"datavalue":{"value":{"id":"Q-stale"}}},
            "qualifiers":{"P582":[{"datavalue":{"value":{"time":"not-a-date"}}}]}
        }]);
        let error = select_current_officeholder_statement(statements.as_array().unwrap())
            .expect_err("a supplied malformed date must fail closed");
        assert!(error.to_string().contains("invalid P582"));
    }

    #[test]
    fn searxng_endpoint_requires_https_except_loopback() {
        assert_eq!(
            validate_searxng_endpoint("http://127.0.0.1:8080/").unwrap(),
            "http://127.0.0.1:8080"
        );
        assert!(validate_searxng_endpoint("https://search.example.org").is_ok());
        assert!(validate_searxng_endpoint("http://search.example.org").is_err());
        assert!(validate_searxng_endpoint("https://192.168.1.10").is_err());
        assert!(validate_searxng_endpoint("https://searx.local").is_err());
        assert!(validate_searxng_endpoint("https://metadata.google.internal").is_err());
        assert!(validate_searxng_endpoint("https://localhost").is_ok());
        assert!(validate_searxng_endpoint("https://user:pass@example.org").is_err());
    }

    #[test]
    fn medlineplus_highlight_markup_is_removed() {
        let xml = br#"<?xml version="1.0"?><nlmSearchResult><list><document url="https://medlineplus.gov/pulmonaryembolism.html"><content name="title">&lt;span class="qt0"&gt;Pulmonary&lt;/span&gt; Embolism</content><content name="snippet">Shortness of &lt;span class="qt1"&gt;breath&lt;/span&gt; &amp; chest pain</content><content name="organizationName">National Library of Medicine</content></document></list></nlmSearchResult>"#;
        let documents = parse_medlineplus(xml).expect("valid MedlinePlus XML");
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].title, "Pulmonary Embolism");
        assert_eq!(documents[0].snippet, "Shortness of breath chest pain");
    }

    #[test]
    fn nasa_3d_catalog_matching_is_deterministic_and_query_specific() {
        let html = r#"
            <a href="https://science.nasa.gov/3d-resources/apollo-11-landing-site/">Apollo</a>
            <a href="https://science.nasa.gov/3d-resources/70-meter-dish/">Dish</a>
            <a href="https://science.nasa.gov/3d-resources/apollo-11-view-of-the-moon/">Moon</a>
        "#;
        let matches = find_nasa_3d_links(html, "Apollo 11 3D model", 5);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].0, "Apollo 11 Landing Site");
        assert!(find_nasa_3d_links(html, "human heart model", 5).is_empty());
    }

    #[test]
    fn country_matching_uses_phrase_boundaries() {
        assert!(contains_phrase("mortality in Oman", "Oman"));
        assert!(!contains_phrase("woman mortality trends", "Oman"));
    }

    #[test]
    fn country_aliases_and_ambiguity_are_explicit() {
        assert!(country_query_matches(
            "population of Russia",
            "Russian Federation"
        ));
        assert!(country_query_matches("GDP in South Korea", "Korea, Rep."));
        assert!(reject_ambiguous_country_query("Georgia population").is_err());
        assert!(reject_ambiguous_country_query("country of Georgia population").is_ok());
        assert!(reject_ambiguous_country_query("Congo population").is_err());
    }

    #[test]
    fn ambiguous_multi_facet_results_stay_in_overview() {
        assert_eq!(
            best_facet(&plan(
                "Kenya research",
                vec![Facet::Overview, Facet::Financials, Facet::Health]
            )),
            Facet::Overview
        );
    }
}
