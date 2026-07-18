use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Facet {
    Overview,
    Financials,
    Safety,
    Locations,
    Health,
    Transmission,
    Textbooks,
    Formulas,
    Statistics,
    News,
    Law,
    Engineering,
    Science,
    Psychology,
    Assets,
}

impl Facet {
    pub const ALL: [Self; 15] = [
        Self::Overview,
        Self::Financials,
        Self::Safety,
        Self::Locations,
        Self::Health,
        Self::Transmission,
        Self::Textbooks,
        Self::Formulas,
        Self::Statistics,
        Self::News,
        Self::Law,
        Self::Engineering,
        Self::Science,
        Self::Psychology,
        Self::Assets,
    ];
}

impl Display for Facet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_value(self)
                .unwrap_or_default()
                .as_str()
                .unwrap_or("overview")
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchRequest {
    pub query: String,
    #[serde(default)]
    pub facets: Vec<Facet>,
    #[serde(default = "default_result_limit")]
    pub result_limit: usize,
    #[serde(default)]
    pub redact_sensitive: bool,
    #[serde(default)]
    pub confirm_sensitive_network: bool,
    #[serde(default)]
    pub approved_plan_id: Option<String>,
    #[serde(default)]
    pub automatic_public_web: bool,
}

const fn default_result_limit() -> usize {
    12
}

impl ResearchRequest {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            facets: Vec::new(),
            result_limit: default_result_limit(),
            redact_sensitive: false,
            confirm_sensitive_network: false,
            approved_plan_id: None,
            automatic_public_web: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPlan {
    pub query: String,
    pub facets: Vec<Facet>,
    pub terms: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchReport {
    pub schema_version: String,
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub query: String,
    pub summary: String,
    pub confidence: Confidence,
    pub evidence: EvidenceAssessment,
    pub plan: ResearchPlan,
    pub findings: Vec<Finding>,
    pub metrics: Vec<Metric>,
    #[serde(default)]
    pub tables: Vec<TableArtifact>,
    pub sources: Vec<SourceRecord>,
    pub warnings: Vec<String>,
    pub run: RunRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableArtifact {
    pub id: String,
    pub title: String,
    pub description: String,
    pub columns: Vec<TableColumn>,
    pub rows: Vec<TableRow>,
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableColumn {
    pub key: String,
    pub label: String,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRow {
    pub id: String,
    pub cells: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceAssessment {
    pub status: EvidenceStatus,
    pub label: String,
    pub explanation: String,
    pub source_coverage: AssessmentGrade,
    pub publisher_diversity: AssessmentGrade,
    pub freshness: AssessmentGrade,
    pub identity_binding: AssessmentGrade,
    pub media_rights: AssessmentGrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    VerifiedIdentity,
    EvidenceAvailable,
    Partial,
    Abstained,
}

impl Display for EvidenceStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::VerifiedIdentity => "verified identity",
            Self::EvidenceAvailable => "evidence available",
            Self::Partial => "partial",
            Self::Abstained => "abstained",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentGrade {
    Strong,
    Moderate,
    Limited,
    NotApplicable,
}

impl Display for AssessmentGrade {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Strong => "strong",
            Self::Moderate => "moderate",
            Self::Limited => "limited",
            Self::NotApplicable => "not applicable",
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Moderate,
    High,
}

impl Display for Confidence {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Low => "low",
                Self::Moderate => "moderate",
                Self::High => "high",
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub title: String,
    pub body: String,
    pub facet: Facet,
    pub confidence: Confidence,
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub content_trust: ContentTrust,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContentTrust {
    CuratedTemplate,
    #[default]
    ExternalUntrusted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub label: String,
    pub value: f64,
    pub display_value: String,
    pub unit: String,
    pub facet: Facet,
    pub source_ids: Vec<String>,
    pub period: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRecord {
    pub id: String,
    pub title: String,
    pub url: String,
    pub publisher: String,
    pub retrieved_at: DateTime<Utc>,
    pub published_at: Option<String>,
    pub license: Option<String>,
    pub source_type: SourceType,
    pub quality: SourceQuality,
    pub content_hash: Option<String>,
    #[serde(default)]
    pub provenance: ProvenanceDetails,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProvenanceDetails {
    pub dataset_id: Option<String>,
    pub request_url: Option<String>,
    pub methodology_url: Option<String>,
    pub observation_period: Option<String>,
    pub source_updated_at: Option<String>,
    pub content_url: Option<String>,
    pub preview_url: Option<String>,
    pub file_format: Option<String>,
    pub file_size_bytes: Option<u64>,
    pub width_pixels: Option<u64>,
    pub height_pixels: Option<u64>,
    pub creator: Option<String>,
    pub credit: Option<String>,
    pub license_url: Option<String>,
    pub alt_text: Option<String>,
    pub media_role: Option<String>,
    pub subject_entity_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Government,
    Intergovernmental,
    Academic,
    News,
    Encyclopedia,
    OpenEducation,
    SearchIndex,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceQuality {
    Primary,
    StrongSecondary,
    DiscoveryOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub engine_version: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub connectors_attempted: Vec<String>,
    pub connectors_succeeded: Vec<String>,
    pub connector_errors: Vec<String>,
    pub network_used: bool,
}

#[derive(Debug, Clone)]
pub struct SourceOutput {
    pub connector: String,
    pub findings: Vec<Finding>,
    pub metrics: Vec<Metric>,
    pub sources: Vec<SourceRecord>,
    pub warnings: Vec<String>,
    pub audit: ConnectorAudit,
}

#[derive(Debug, Clone, Default)]
pub struct ConnectorAudit {
    pub attempted: Vec<String>,
    pub succeeded: Vec<String>,
    pub errors: Vec<String>,
}
