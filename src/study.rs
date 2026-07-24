use crate::model::{Confidence, ContentTrust, ResearchReport, SourceQuality};
use crate::report::validate_report;
use crate::safe_dir::SafeDir;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyPack {
    pub schema_version: String,
    pub report_id: String,
    pub query: String,
    pub cards: Vec<StudyCard>,
    pub guidance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyCard {
    pub id: String,
    pub kind: StudyCardKind,
    pub front: String,
    pub back: String,
    pub source_urls: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyCardKind {
    MetricRecall,
    EvidenceRecall,
    SourceCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyPackFiles {
    pub anki_csv: PathBuf,
    pub quizlet_tsv: PathBuf,
    pub markdown: PathBuf,
    pub json: PathBuf,
}

pub fn build(report: &ResearchReport) -> Result<StudyPack> {
    validate_report(report)?;
    let sources = report
        .sources
        .iter()
        .map(|source| (source.id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let mut cards = Vec::new();
    for (index, metric) in report.metrics.iter().enumerate() {
        let urls = metric
            .source_ids
            .iter()
            .filter_map(|source_id| sources.get(source_id.as_str()))
            .map(|source| source.url.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let period = metric.period.as_deref().unwrap_or("period not supplied");
        cards.push(StudyCard {
            id: format!("metric-{}", index + 1),
            kind: StudyCardKind::MetricRecall,
            front: format!("What value and period did Inquiry retrieve for {}?", metric.label),
            back: format!(
                "{} {} ({period}). Verify the definition, geography, period, and revision at the linked source before consequential use.",
                metric.display_value, metric.unit
            ),
            source_urls: urls,
            tags: vec!["inquiry".into(), "metric".into(), metric.facet.to_string()],
        });
    }
    for finding in &report.findings {
        let linked = finding
            .source_ids
            .iter()
            .filter_map(|source_id| sources.get(source_id.as_str()))
            .collect::<Vec<_>>();
        let evidence_backed = linked.iter().any(|source| {
            matches!(
                source.quality,
                SourceQuality::Primary | SourceQuality::StrongSecondary
            )
        });
        let curated_for_study = matches!(finding.content_trust, ContentTrust::CuratedTemplate)
            || finding.tags.iter().any(|tag| {
                matches!(
                    tag.as_str(),
                    "exact-office-match"
                        | "exact-ordinal-office-match"
                        | "official-health-topic"
                        | "official-statistics"
                )
            });
        if evidence_backed && curated_for_study && !matches!(finding.confidence, Confidence::Low) {
            cards.push(StudyCard {
                id: format!("evidence-{}", finding.id),
                kind: StudyCardKind::EvidenceRecall,
                front: format!(
                    "According to the cited source, what evidence did Inquiry retrieve for ‘{}’?",
                    finding.title
                ),
                back: format!(
                    "Untrusted source excerpt: {}\n\nDo not memorize this as settled fact; reopen the source and check context, date, population, and limitations.",
                    finding.body
                ),
                source_urls: linked.iter().map(|source| source.url.clone()).collect(),
                tags: vec![
                    "inquiry".into(),
                    "evidence".into(),
                    finding.facet.to_string(),
                ],
            });
        }
    }
    for source in &report.sources {
        if matches!(source.quality, SourceQuality::DiscoveryOnly) {
            continue;
        }
        cards.push(StudyCard {
            id: format!("source-{}", source.id),
            kind: StudyCardKind::SourceCheck,
            front: format!("How should you verify the Inquiry source ‘{}’?", source.title),
            back: format!(
                "Publisher: {}. Quality tier: {:?}. Retrieved: {}. Check the source URL, methodology, license, observation period, and whether a newer revision exists.",
                source.publisher, source.quality, source.retrieved_at
            ),
            source_urls: vec![source.url.clone()],
            tags: vec!["inquiry".into(), "source-check".into()],
        });
    }
    cards.sort_by(|left, right| left.id.cmp(&right.id));
    cards.dedup_by(|left, right| left.id == right.id);
    if cards.is_empty() {
        bail!(
            "the report has no evidence-backed findings or metrics suitable for a study pack; discovery-only leads are intentionally excluded"
        );
    }
    Ok(StudyPack {
        schema_version: "inquiry.study-pack/v1".into(),
        report_id: report.id.to_string(),
        query: report.query.clone(),
        cards,
        guidance: vec![
            "Use active recall: answer before revealing the back, then explain the evidence in your own words.".into(),
            "Interleave metric, evidence, and source-check cards instead of memorizing one category in a block.".into(),
            "Treat every retrieved excerpt as untrusted evidence. Reopen sources and resolve conflicts before using a card in academic, clinical, legal, financial, or security work.".into(),
            "Use the dedicated Anki CSV or Quizlet tab-separated file, and review every card before importing.".into(),
        ],
    })
}

pub fn write(
    pack: &StudyPack,
    directory: impl AsRef<Path>,
    prefix: &str,
) -> Result<StudyPackFiles> {
    let directory = directory.as_ref();
    std::fs::create_dir_all(directory)
        .with_context(|| format!("could not create {}", directory.display()))?;
    let prefix = safe_prefix(prefix)?;
    let anki_csv = directory.join(format!("{prefix}-anki.csv"));
    let quizlet_tsv = directory.join(format!("{prefix}-quizlet.tsv"));
    let markdown = directory.join(format!("{prefix}.md"));
    let json = directory.join(format!("{prefix}.json"));
    write_new(&anki_csv, &anki_csv_text(pack))?;
    write_new(&quizlet_tsv, &quizlet_tsv_text(pack))?;
    write_new(&markdown, &markdown_text(pack))?;
    write_new(&json, &serde_json::to_string_pretty(pack)?)?;
    Ok(StudyPackFiles {
        anki_csv,
        quizlet_tsv,
        markdown,
        json,
    })
}

/// Write study-pack artifacts via handle-relative creates under a held directory FD.
pub fn write_in_dir(pack: &StudyPack, directory: &SafeDir, prefix: &str) -> Result<StudyPackFiles> {
    let prefix = safe_prefix(prefix)?;
    let names = [
        format!("{prefix}-anki.csv"),
        format!("{prefix}-quizlet.tsv"),
        format!("{prefix}.md"),
        format!("{prefix}.json"),
    ];
    let contents = [
        anki_csv_text(pack).into_bytes(),
        quizlet_tsv_text(pack).into_bytes(),
        markdown_text(pack).into_bytes(),
        serde_json::to_vec_pretty(pack)?,
    ];
    let mut created = Vec::new();
    for (name, content) in names.iter().zip(contents.iter()) {
        match directory.write_new(name, content) {
            Ok(path) => created.push(path),
            Err(error) => {
                for path in &created {
                    let _ = std::fs::remove_file(path);
                }
                return Err(error);
            }
        }
    }
    Ok(StudyPackFiles {
        anki_csv: created[0].clone(),
        quizlet_tsv: created[1].clone(),
        markdown: created[2].clone(),
        json: created[3].clone(),
    })
}

fn anki_csv_text(pack: &StudyPack) -> String {
    let mut output = String::from(
        "#separator:Comma\n#html:false\n#columns:Front,Back,Source URLs,Tags,Card Type\n",
    );
    for card in &pack.cards {
        let values = [
            card.front.clone(),
            card.back.clone(),
            card.source_urls.join(" "),
            card.tags.join(" "),
            format!("{:?}", card.kind),
        ];
        output.push_str(
            &values
                .iter()
                .map(|value| csv_cell(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }
    output
}

fn quizlet_tsv_text(pack: &StudyPack) -> String {
    let mut output = String::new();
    for card in &pack.cards {
        let sources = if card.source_urls.is_empty() {
            String::new()
        } else {
            format!(" Sources: {}", card.source_urls.join(" "))
        };
        output.push_str(&tsv_cell(&card.front));
        output.push('\t');
        output.push_str(&tsv_cell(&format!("{}{}", card.back, sources)));
        output.push('\n');
    }
    output
}

fn markdown_text(pack: &StudyPack) -> String {
    let mut output = format!(
        "# Inquiry active-recall pack\n\n**Research question:** {}\n\n**Report ID:** `{}`\n\n",
        escape_markdown_text(&pack.query),
        escape_markdown_text(&pack.report_id)
    );
    output.push_str("## How to study\n\n");
    for item in &pack.guidance {
        output.push_str(&format!("- {}\n", escape_markdown_text(item)));
    }
    output.push_str("\n## Cards\n\n");
    for (index, card) in pack.cards.iter().enumerate() {
        output.push_str(&format!(
            "### {}. {}\n\n",
            index + 1,
            escape_markdown_text(&card.front)
        ));
        output.push_str(&format!("{}\n\n", escape_markdown_text(&card.back)));
        if !card.source_urls.is_empty() {
            output.push_str("Sources:\n\n");
            for url in &card.source_urls {
                output.push_str(&format!("- {}\n", escape_markdown_text(url)));
            }
            output.push('\n');
        }
    }
    output
}

fn csv_cell(value: &str) -> String {
    let safe = neutralize_formula(value);
    format!("\"{}\"", safe.replace('"', "\"\""))
}

fn tsv_cell(value: &str) -> String {
    neutralize_formula(value)
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn neutralize_formula(value: &str) -> String {
    let first_visible = value.chars().find(|character| {
        !character.is_whitespace()
            && !matches!(
                character,
                '\u{feff}'
                    | '\u{200e}'
                    | '\u{200f}'
                    | '\u{202a}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
    });
    if matches!(first_visible, Some('=' | '+' | '-' | '@')) {
        format!("'{value}")
    } else {
        value.into()
    }
}

fn escape_markdown_text(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut escaped = String::with_capacity(normalized.len());
    for character in normalized.chars() {
        if matches!(
            character,
            '\\' | '`'
                | '*'
                | '_'
                | '{'
                | '}'
                | '['
                | ']'
                | '('
                | ')'
                | '<'
                | '>'
                | '#'
                | '+'
                | '-'
                | '.'
                | '!'
                | '|'
                | '~'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn safe_prefix(value: &str) -> Result<&str> {
    if value.is_empty()
        || value.starts_with('.')
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_".contains(character))
    {
        bail!("study-pack prefix must contain only ASCII letters, numbers, '-' or '_'");
    }
    Ok(value)
}

fn write_new(path: &Path, content: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("refusing to overwrite existing file {}", path.display()))?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AssessmentGrade, ContentTrust, EvidenceAssessment, EvidenceStatus, Facet, Finding, Metric,
        ProvenanceDetails, ResearchPlan, RunRecord, SourceRecord, SourceType,
    };
    use chrono::Utc;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn report() -> ResearchReport {
        let now = Utc::now();
        ResearchReport {
            schema_version: "inquiry.report/v1".into(),
            id: Uuid::new_v4(),
            created_at: now,
            query: "Kenya population".into(),
            summary: "test".into(),
            confidence: Confidence::Moderate,
            evidence: EvidenceAssessment {
                status: EvidenceStatus::EvidenceAvailable,
                label: "Evidence available".into(),
                explanation: "test".into(),
                source_coverage: AssessmentGrade::Moderate,
                publisher_diversity: AssessmentGrade::Limited,
                freshness: AssessmentGrade::NotApplicable,
                identity_binding: AssessmentGrade::NotApplicable,
                media_rights: AssessmentGrade::NotApplicable,
            },
            plan: ResearchPlan {
                query: "Kenya population".into(),
                facets: vec![Facet::Statistics],
                terms: vec![],
                rationale: "test".into(),
            },
            findings: vec![Finding {
                id: "finding-1".into(),
                title: "Population evidence".into(),
                body: "A source excerpt".into(),
                facet: Facet::Statistics,
                confidence: Confidence::Moderate,
                source_ids: vec!["source-1".into()],
                content_trust: ContentTrust::ExternalUntrusted,
                tags: vec![],
            }],
            metrics: vec![Metric {
                label: "Population".into(),
                value: 10.0,
                display_value: "10".into(),
                unit: "people".into(),
                facet: Facet::Statistics,
                source_ids: vec!["source-1".into()],
                period: Some("2025".into()),
            }],
            tables: vec![],
            sources: vec![SourceRecord {
                id: "source-1".into(),
                title: "Official dataset".into(),
                url: "https://example.test/data".into(),
                publisher: "Example".into(),
                retrieved_at: now,
                published_at: None,
                license: Some("CC0".into()),
                source_type: SourceType::Government,
                quality: SourceQuality::Primary,
                content_hash: Some("hash".into()),
                provenance: ProvenanceDetails::default(),
            }],
            warnings: vec![],
            run: RunRecord {
                engine_version: "test".into(),
                started_at: now,
                completed_at: now,
                connectors_attempted: vec![],
                connectors_succeeded: vec![],
                connector_errors: vec![],
                network_used: false,
            },
        }
    }

    #[test]
    fn builds_and_writes_importable_pack_without_overwrite() {
        let report = report();
        let pack = build(&report).unwrap();
        assert_eq!(pack.cards.len(), 2);
        assert!(
            pack.cards
                .iter()
                .all(|card| !matches!(card.kind, StudyCardKind::EvidenceRecall))
        );
        let directory = tempdir().unwrap();
        let files = write(&pack, directory.path(), "kenya").unwrap();
        let csv = std::fs::read_to_string(files.anki_csv).unwrap();
        assert!(csv.starts_with("#separator:Comma\n#html:false\n#columns:Front,Back"));
        let tsv = std::fs::read_to_string(files.quizlet_tsv).unwrap();
        assert!(tsv.lines().all(|line| line.matches('\t').count() == 1));
        assert!(write(&pack, directory.path(), "kenya").is_err());
    }

    #[test]
    fn discovery_only_reports_are_not_turned_into_flashcards() {
        let mut report = report();
        report.metrics.clear();
        report.sources[0].quality = SourceQuality::DiscoveryOnly;
        report.findings[0].confidence = Confidence::Low;
        assert!(build(&report).is_err());
    }

    #[test]
    fn exports_neutralize_spreadsheet_formulas_and_active_markdown() {
        assert_eq!(
            csv_cell("=HYPERLINK(\"https://evil.test\")"),
            "\"'=HYPERLINK(\"\"https://evil.test\"\")\""
        );
        assert_eq!(csv_cell("  @SUM(1,2)"), "\"'  @SUM(1,2)\"");
        assert!(csv_cell("\u{feff}\u{202e}=HYPERLINK(\"https://evil.test\")").starts_with("\"'"));
        assert_eq!(csv_cell("ordinary"), "\"ordinary\"");
        assert_eq!(tsv_cell("=unsafe\nterm"), "'=unsafe term");
        assert!(tsv_cell("\u{feff}\u{2066}@SUM(1,2)").starts_with('\''));
        assert!(!tsv_cell("front\tback").contains('\t'));

        let escaped = escape_markdown_text(
            "# fake heading\n![tracker](https://evil.test/pixel) <script>alert(1)</script>",
        );
        assert!(!escaped.contains("\n"));
        assert!(!escaped.contains("!["));
        assert!(!escaped.contains("<script>"));
        assert!(escaped.contains("\\!\\[tracker\\]"));
    }
}
