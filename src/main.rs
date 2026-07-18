use anyhow::{Context, Result, bail};
use barnlabs_inquiry::aviation::{self, FlightCarrier};
use barnlabs_inquiry::capabilities;
use barnlabs_inquiry::convert;
use barnlabs_inquiry::engine::{EngineConfig, ResearchEngine};
use barnlabs_inquiry::formula;
use barnlabs_inquiry::live;
use barnlabs_inquiry::math;
use barnlabs_inquiry::mcp;
use barnlabs_inquiry::medication;
use barnlabs_inquiry::model::{Facet, ResearchRequest};
use barnlabs_inquiry::package::{self, PackageCarrier};
use barnlabs_inquiry::place;
use barnlabs_inquiry::privacy;
use barnlabs_inquiry::report;
use barnlabs_inquiry::study;
use barnlabs_inquiry::study_local::{self, IndexOptions};
use barnlabs_inquiry::timeline::{self, TimelineArtifact};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
#[command(
    name = "inquiry",
    version,
    about = "BarnLabs Inquiry — ask broadly, verify deeply"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show Inquiry's reviewed capability, coverage, and abstention matrix.
    Capabilities,
    /// Build a local connector and privacy execution plan without networking.
    Plan {
        query: Vec<String>,
        /// Read the query from standard input so it is not placed in process arguments.
        #[arg(long)]
        stdin: bool,
    },
    /// Retrieve one approved, bounded NASA EONET natural-event snapshot.
    LiveEvents {
        /// Suppress networking and fail closed even if approval is supplied.
        #[arg(long)]
        offline: bool,
        /// Approve the exact fingerprint produced by a LiveEvents `inquiry plan`.
        #[arg(long)]
        approved_plan: Option<String>,
        /// Approve only while the fixed EONET plan remains eligible public web.
        #[arg(long, conflicts_with = "approved_plan")]
        automatic_public_web: bool,
    },
    /// Research a natural-language question using public sources.
    Research {
        query: Vec<String>,
        /// Read the query from standard input so sensitive text is not placed in process arguments.
        #[arg(long)]
        stdin: bool,
        #[arg(long = "facet")]
        facets: Vec<String>,
        /// Maximum records requested from each matching connector. The final
        /// report can contain more findings when several connectors respond.
        #[arg(long, default_value_t = 12)]
        limit: usize,
        #[arg(long, value_enum, default_value_t=OutputFormat::Html)]
        format: OutputFormat,
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(long)]
        open: bool,
        #[arg(long)]
        offline: bool,
        /// Remove common identifiers locally before live connector routing.
        #[arg(long, conflicts_with = "confirm_sensitive_web")]
        redact_sensitive: bool,
        /// Explicitly approve sending a query flagged as sensitive (never
        /// permits secrets, restricted data, or highly sensitive identifiers).
        #[arg(long, conflicts_with = "redact_sensitive")]
        confirm_sensitive_web: bool,
        /// Approve the exact local plan fingerprint produced by `inquiry plan` for this run.
        #[arg(long)]
        approved_plan: Option<String>,
        /// Automatically approve only low-risk public-query plans. Sensitive and identifier plans still fail closed.
        #[arg(long, conflicts_with = "approved_plan")]
        automatic_public_web: bool,
    },
    /// Inspect a query locally for common sensitive-data patterns without networking.
    PrivacyCheck {
        query: Vec<String>,
        /// Read the query from standard input so sensitive text is not placed in process arguments.
        #[arg(long)]
        stdin: bool,
    },
    /// Retrieve one airport's current FAA traffic-management events.
    AirportStatus {
        /// Three-letter U.S. airport identifier, such as ATL or JFK.
        airport: String,
        #[arg(long)]
        offline: bool,
    },
    /// Prepare a no-network handoff to an official airline flight-status page.
    FlightStatus {
        #[arg(value_enum)]
        carrier: FlightCarrier,
        /// Exact carrier flight identifier, such as AA123.
        flight: String,
        /// Optional scheduled date in YYYY-MM-DD form.
        #[arg(long)]
        date: Option<String>,
    },
    /// Look up one N-number in a user-downloaded FAA releasable registry archive.
    AircraftLookup {
        n_number: String,
        /// Local .zip archive downloaded from the official FAA releasable database page.
        #[arg(long)]
        registry: PathBuf,
    },
    /// Prepare a no-network handoff to an official carrier tracking page.
    PackageTracking {
        #[arg(value_enum)]
        carrier: PackageCarrier,
        /// Tracking identifier. Prefer --stdin to keep it out of process arguments.
        tracking_identifier: Option<String>,
        /// Read the tracking identifier from standard input.
        #[arg(long)]
        stdin: bool,
        /// Put the identifier in the official carrier URL after explicit opt-in.
        #[arg(long)]
        deep_link: bool,
    },
    /// Convert between supported units.
    Convert {
        #[arg(allow_hyphen_values = true)]
        value: f64,
        from: String,
        to: String,
        #[arg(long)]
        json: bool,
    },
    /// Evaluate a deterministic mathematical expression.
    Calculate {
        #[arg(allow_hyphen_values = true)]
        expression: Vec<String>,
    },
    /// Summarize finite numeric values with documented statistical conventions.
    Stats {
        #[arg(allow_hyphen_values = true)]
        values: Vec<f64>,
    },
    /// Estimate a derivative at x with a central finite difference.
    Differentiate {
        #[arg(allow_hyphen_values = true)]
        expression: String,
        #[arg(long, allow_hyphen_values = true)]
        at: f64,
        #[arg(long, allow_hyphen_values = true)]
        step: Option<f64>,
    },
    /// Estimate a definite integral with composite Simpson's rule.
    Integrate {
        #[arg(allow_hyphen_values = true)]
        expression: String,
        #[arg(long, allow_hyphen_values = true)]
        from: f64,
        #[arg(long, allow_hyphen_values = true)]
        to: f64,
        #[arg(long, default_value_t = 1_000)]
        intervals: usize,
    },
    /// Generate a self-contained interactive-browser SVG graph and sample table.
    Graph {
        #[arg(allow_hyphen_values = true)]
        expression: String,
        #[arg(long, allow_hyphen_values = true)]
        from: f64,
        #[arg(long, allow_hyphen_values = true)]
        to: f64,
        #[arg(long, default_value_t = 401)]
        samples: usize,
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(long)]
        open: bool,
    },
    /// Retrieve current FDA-submitted label sections for one or two named medications.
    MedicationEvidence {
        drug: String,
        #[arg(long = "with")]
        with_drug: Option<String>,
        #[arg(long, default_value_t = 2)]
        limit: usize,
        #[arg(long)]
        offline: bool,
    },
    /// List or inspect reviewed formulas.
    Formula {
        name: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Resolve exact place candidates, optionally ranked relative to a nearby landmark.
    ResolvePlace {
        query: Vec<String>,
        #[arg(long, default_value_t = 8)]
        limit: usize,
        #[arg(long)]
        offline: bool,
    },
    /// Render a report JSON document read from standard input without rerunning research.
    RenderReport {
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Build dedicated Anki CSV and Quizlet TSV exports plus Markdown and JSON.
    StudyPack {
        #[arg(long, default_value = "reports")]
        out_dir: PathBuf,
        #[arg(long)]
        prefix: Option<String>,
    },
    /// Build a private, deterministic search index from one explicitly selected course folder.
    StudyIndex {
        directory: Option<PathBuf>,
        /// Read directory, output, and metadata as JSON from standard input. This keeps
        /// local paths and course labels out of process arguments for desktop clients.
        #[arg(long)]
        request_stdin: bool,
        /// Destination for the private index. Existing files are never overwritten.
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        course: Option<String>,
        #[arg(long)]
        instructor: Option<String>,
        /// Include PowerPoint speaker notes. They are excluded by default because they may
        /// contain unpublished or hidden material.
        #[arg(long)]
        include_speaker_notes: bool,
    },
    /// Search an InquiryStudy index locally and return exact source excerpts with hashes.
    StudySearch {
        index: PathBuf,
        query: Vec<String>,
        /// Read the query from standard input so local course terms stay out of process arguments.
        #[arg(long)]
        stdin: bool,
        #[arg(long, default_value_t = 8)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Build Anki, Quizlet, Markdown, and JSON recall files from cited local-note matches.
    StudyLocalPack {
        index: PathBuf,
        query: Vec<String>,
        /// Read the query from standard input so local course terms stay out of process arguments.
        #[arg(long)]
        stdin: bool,
        #[arg(long, default_value_t = 12)]
        limit: usize,
        #[arg(long, default_value = "reports")]
        out_dir: PathBuf,
        #[arg(long, default_value = "inquiry-local-study")]
        prefix: String,
    },
    /// Render a validated, source-cited interactive timeline JSON document from standard input.
    RenderTimeline {
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(long)]
        open: bool,
    },
    /// Start the Model Context Protocol server over standard I/O.
    Mcp {
        #[arg(long)]
        offline: bool,
    },
    /// Generate a representative offline report without network access.
    Demo {
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(long)]
        open: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Html,
    Json,
    Summary,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Capabilities => {
            println!("{}", serde_json::to_string_pretty(&capabilities::matrix())?);
        }
        Commands::Plan { query, stdin } => {
            let query = if stdin {
                read_standard_input(16_384)?
            } else {
                query.join(" ")
            };
            if query.trim().is_empty() {
                bail!("provide a research question after `inquiry plan`");
            }
            let execution_plan = if live::is_live_events_intent(&query) {
                live::eonet_open_execution_plan()
            } else {
                let request = ResearchRequest::new(query);
                let engine = ResearchEngine::new(EngineConfig {
                    network: true,
                    searxng_url: std::env::var("INQUIRY_SEARXNG_URL").ok(),
                })?;
                engine.execution_plan(&request)
            };
            println!("{}", serde_json::to_string_pretty(&execution_plan)?);
        }
        Commands::LiveEvents {
            offline,
            approved_plan,
            automatic_public_web,
        } => {
            let snapshot = live::fetch_eonet_open_snapshot(
                offline,
                approved_plan.as_deref(),
                automatic_public_web,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
        }
        Commands::Research {
            query,
            stdin,
            facets,
            limit,
            format,
            out,
            open,
            offline,
            redact_sensitive,
            confirm_sensitive_web,
            approved_plan,
            automatic_public_web,
        } => {
            let query = if stdin {
                read_standard_input(16_384)?
            } else {
                query.join(" ")
            };
            if query.trim().is_empty() {
                bail!("provide a research question after `inquiry research`");
            }
            let mut request = ResearchRequest::new(query.clone());
            request.result_limit = limit.clamp(1, 25);
            request.redact_sensitive = redact_sensitive;
            request.confirm_sensitive_network = confirm_sensitive_web;
            request.approved_plan_id = approved_plan;
            request.automatic_public_web = automatic_public_web;
            request.facets = facets
                .iter()
                .map(|facet| parse_facet(facet))
                .collect::<Result<Vec<_>>>()?;
            let engine = ResearchEngine::new(EngineConfig {
                network: !offline,
                searxng_url: std::env::var("INQUIRY_SEARXNG_URL").ok(),
            })?;
            let report = engine.research(request).await?;
            match format {
                OutputFormat::Html => {
                    let path = out.unwrap_or_else(|| {
                        report::default_report_path(&query, report.id, report.created_at, "html")
                    });
                    report::write_html(&report, &path)?;
                    println!("{}", path.canonicalize().unwrap_or(path.clone()).display());
                    if open {
                        open_path(&path)?;
                    }
                }
                OutputFormat::Json => {
                    if let Some(path) = out {
                        report::write_json(&report, &path)?;
                        println!("{}", path.canonicalize().unwrap_or(path.clone()).display());
                    } else {
                        println!("{}", serde_json::to_string_pretty(&report)?);
                    }
                }
                OutputFormat::Summary => {
                    println!(
                        "{}\n\nEvidence assessment: {}\n{}\nSource coverage: {}\nFindings: {} · Metrics: {} · Sources: {}",
                        report.summary,
                        report.evidence.label,
                        report.evidence.explanation,
                        report.evidence.source_coverage,
                        report.findings.len(),
                        report.metrics.len(),
                        report.sources.len()
                    );
                    for warning in report.warnings {
                        println!("WARNING: {warning}");
                    }
                }
            }
        }
        Commands::PrivacyCheck { query, stdin } => {
            let query = if stdin {
                read_standard_input(16_384)?
            } else {
                query.join(" ")
            };
            if query.trim().is_empty() {
                bail!("provide text to inspect after `inquiry privacy-check` or use --stdin");
            }
            if query.chars().count() > 4_000 {
                bail!("privacy-check input must not exceed 4,000 characters");
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&privacy::assess(&query))?
            );
        }
        Commands::AirportStatus { airport, offline } => {
            let status = aviation::airport_status(&airport, !offline).await?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Commands::FlightStatus {
            carrier,
            flight,
            date,
        } => {
            let handoff = aviation::flight_status_handoff(carrier, &flight, date.as_deref())?;
            println!("{}", serde_json::to_string_pretty(&handoff)?);
        }
        Commands::AircraftLookup { n_number, registry } => {
            let registration = aviation::aircraft_registration_lookup(registry, &n_number)?;
            println!("{}", serde_json::to_string_pretty(&registration)?);
        }
        Commands::PackageTracking {
            carrier,
            tracking_identifier,
            stdin,
            deep_link,
        } => {
            if stdin && tracking_identifier.is_some() {
                bail!(
                    "provide the tracking identifier either as an argument or with --stdin, not both"
                );
            }
            let tracking_identifier = if stdin {
                read_standard_input(256)?
            } else {
                tracking_identifier.context(
                    "provide a tracking identifier or use --stdin to keep it out of process arguments",
                )?
            };
            let handoff =
                package::tracking_handoff(carrier, tracking_identifier.trim(), deep_link)?;
            println!("{}", serde_json::to_string_pretty(&handoff)?);
        }
        Commands::Convert {
            value,
            from,
            to,
            json,
        } => {
            let result = convert::convert(value, &from, &to)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!(
                    "{} {} = {:.10} {}\n{}",
                    result.input_value,
                    result.input_unit,
                    result.output_value,
                    result.output_unit,
                    result.formula
                );
            }
        }
        Commands::Calculate { expression } => {
            let result = math::evaluate(&expression.join(" "))?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Stats { values } => {
            let result = math::summarize(&values)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Differentiate {
            expression,
            at,
            step,
        } => {
            let result = math::differentiate(&expression, at, step)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Integrate {
            expression,
            from,
            to,
            intervals,
        } => {
            let result = math::integrate(&expression, from, to, intervals)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Graph {
            expression,
            from,
            to,
            samples,
            out,
            open,
        } => {
            let graph = math::graph(&expression, from, to, samples)?;
            let path = out.unwrap_or_else(|| math::default_graph_path(&expression));
            math::write_graph_html(&graph, &path)?;
            println!("{}", path.canonicalize().unwrap_or(path.clone()).display());
            if open {
                open_path(&path)?;
            }
        }
        Commands::MedicationEvidence {
            drug,
            with_drug,
            limit,
            offline,
        } => {
            let mut names = vec![drug];
            if let Some(other) = with_drug {
                names.push(other);
            }
            let evidence = medication::retrieve(&names, limit, !offline).await?;
            println!("{}", serde_json::to_string_pretty(&evidence)?);
        }
        Commands::Formula { name, json } => match name {
            Some(name) => {
                let found = formula::find(&name)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(found)?);
                } else {
                    println!(
                        "{}\n{}\n{}\nCaveat: {}\nReferences:\n{}",
                        found.name,
                        found.expression,
                        found.description,
                        found.caveat,
                        found
                            .references
                            .iter()
                            .map(|reference| format!("- {reference}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    );
                }
            }
            None => {
                if json {
                    println!("{}", serde_json::to_string_pretty(formula::FORMULAS)?);
                } else {
                    for item in formula::FORMULAS {
                        println!("{:<18} {}", item.id, item.name);
                    }
                }
            }
        },
        Commands::ResolvePlace {
            query,
            limit,
            offline,
        } => {
            let resolution = place::resolve(&query.join(" "), limit, !offline).await?;
            println!("{}", serde_json::to_string_pretty(&resolution)?);
        }
        Commands::RenderReport { out } => {
            let input = read_standard_input(8_000_000)?;
            let parsed = serde_json::from_str(&input)
                .context("standard input was not a valid Inquiry report")?;
            report::validate_report(&parsed)?;
            let path = out.unwrap_or_else(|| {
                report::default_report_path(&parsed.query, parsed.id, parsed.created_at, "html")
            });
            report::write_html(&parsed, &path)?;
            println!("{}", path.canonicalize().unwrap_or(path).display());
        }
        Commands::StudyPack { out_dir, prefix } => {
            let input = read_standard_input(8_000_000)?;
            let parsed = serde_json::from_str(&input)
                .context("standard input was not a valid Inquiry report")?;
            let pack = study::build(&parsed)?;
            let prefix = prefix.unwrap_or_else(|| format!("study-{}", &pack.report_id[..8]));
            let files = study::write(&pack, out_dir, &prefix)?;
            println!("{}", serde_json::to_string_pretty(&files)?);
        }
        Commands::StudyIndex {
            directory,
            request_stdin,
            out,
            course,
            instructor,
            include_speaker_notes,
        } => {
            let request = if request_stdin {
                if directory.is_some()
                    || out.is_some()
                    || course.is_some()
                    || instructor.is_some()
                    || include_speaker_notes
                {
                    bail!(
                        "--request-stdin cannot be combined with directory, --out, --course, --instructor, or --include-speaker-notes"
                    );
                }
                serde_json::from_str::<StudyIndexRequest>(&read_standard_input(16_384)?)
                    .context("standard input was not a valid InquiryStudy index request")?
            } else {
                StudyIndexRequest {
                    directory: directory
                        .context("provide a course directory or use --request-stdin")?,
                    out,
                    course,
                    instructor,
                    include_speaker_notes,
                }
            };
            let StudyIndexRequest {
                directory,
                out,
                course,
                instructor,
                include_speaker_notes,
            } = request;
            let destination = out.unwrap_or_else(|| {
                let label = directory
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("course");
                PathBuf::from("reports")
                    .join(format!("{}-study-index.json", safe_filename_label(label)))
            });
            let excluded_paths = destination
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map(Path::to_path_buf)
                .into_iter()
                .collect();
            let index = study_local::index_directory(
                &directory,
                &IndexOptions {
                    course,
                    instructor,
                    include_speaker_notes,
                    excluded_paths,
                },
            )?;
            let path = study_local::write_index(&index, &destination)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "path": path.canonicalize().unwrap_or(path),
                    "documents_indexed": index.documents.len(),
                    "segments_indexed": index.segments.len(),
                    "files_skipped": index.skipped.len(),
                    "skipped": index.skipped,
                    "warnings": index.warnings,
                    "application_network_requests": 0,
                    "notice": "The index contains private source excerpts. Store, sync, and share it with the same care as the original course material. macOS or a configured File Provider may independently hydrate cloud-only inputs or sync the output location."
                }))?
            );
        }
        Commands::StudySearch {
            index,
            query,
            stdin,
            limit,
            json,
        } => {
            let query = if stdin {
                read_standard_input(4_000)?
            } else {
                query.join(" ")
            };
            let index = study_local::read_index(index)?;
            let result = study_local::search(&index, &query, limit)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                if result.results.is_empty() {
                    println!("No cited local excerpts matched “{}”.", result.query);
                }
                for item in &result.results {
                    println!(
                        "{}. {} — {} [score {:.3}; sha256 {}]\n{}\n",
                        item.rank,
                        item.relative_path,
                        item.locator,
                        item.score,
                        item.content_hash,
                        item.excerpt
                    );
                }
                for warning in result.warnings {
                    println!("NOTICE: {warning}");
                }
            }
        }
        Commands::StudyLocalPack {
            index,
            query,
            stdin,
            limit,
            out_dir,
            prefix,
        } => {
            let query = if stdin {
                read_standard_input(4_000)?
            } else {
                query.join(" ")
            };
            let index = study_local::read_index(index)?;
            let result = study_local::search(&index, &query, limit)?;
            let pack = study_local::build_recall_pack(&result)?;
            let files = study_local::write_recall_pack(&pack, out_dir, &prefix)?;
            println!("{}", serde_json::to_string_pretty(&files)?);
        }
        Commands::RenderTimeline { out, open } => {
            let input = read_standard_input(2_000_000)?;
            let artifact: TimelineArtifact = serde_json::from_str(&input)
                .context("standard input was not valid Inquiry timeline JSON")?;
            let path = out.unwrap_or_else(|| {
                PathBuf::from("reports")
                    .join(format!("timeline-{}.html", uuid::Uuid::new_v4().simple()))
            });
            let written = timeline::write_html(&artifact, &path)?;
            println!(
                "{}",
                written.canonicalize().unwrap_or(written.clone()).display()
            );
            if open {
                open_path(&written)?;
            }
        }
        Commands::Mcp { offline } => {
            mcp::run_stdio(EngineConfig {
                network: !offline,
                searxng_url: std::env::var("INQUIRY_SEARXNG_URL").ok(),
            })
            .await?
        }
        Commands::Demo { out, open } => {
            let query = "Compare dengue transmission, public-health safety metrics, and open statistics for researchers";
            let engine = ResearchEngine::new(EngineConfig {
                network: false,
                searxng_url: None,
            })?;
            let report = engine.research(ResearchRequest::new(query)).await?;
            let path = out.unwrap_or_else(|| {
                report::default_report_path(query, report.id, report.created_at, "html")
            });
            report::write_html(&report, &path)?;
            println!("{}", path.canonicalize().unwrap_or(path.clone()).display());
            if open {
                open_path(&path)?;
            }
        }
    }
    Ok(())
}

fn read_standard_input(max_bytes: usize) -> Result<String> {
    let mut input = String::new();
    std::io::stdin()
        .take(max_bytes as u64 + 1)
        .read_to_string(&mut input)
        .context("could not read standard input")?;
    if input.len() > max_bytes {
        bail!("standard input exceeds the {max_bytes}-byte limit");
    }
    if input.trim().is_empty() {
        bail!("standard input was empty");
    }
    Ok(input)
}

fn parse_facet(input: &str) -> Result<Facet> {
    Ok(match input.to_lowercase().as_str() {
        "overview" => Facet::Overview,
        "financials" | "finance" => Facet::Financials,
        "safety" => Facet::Safety,
        "locations" | "location" => Facet::Locations,
        "health" | "diseases" | "disease" => Facet::Health,
        "transmission" | "transmissions" => Facet::Transmission,
        "textbooks" | "books" => Facet::Textbooks,
        "formulas" | "formula" => Facet::Formulas,
        "statistics" | "stats" => Facet::Statistics,
        "news" => Facet::News,
        "law" | "legal" => Facet::Law,
        "engineering" | "standards" => Facet::Engineering,
        "science" | "chemistry" => Facet::Science,
        "psychology" | "psych" => Facet::Psychology,
        "assets" | "models" | "3d" => Facet::Assets,
        _ => bail!("unknown facet '{input}'"),
    })
}

fn open_path(path: &Path) -> Result<()> {
    let absolute = path
        .canonicalize()
        .with_context(|| format!("could not resolve {}", path.display()))?;
    let status = if cfg!(target_os = "macos") {
        Command::new("open").arg(&absolute).status()?
    } else if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(&absolute)
            .status()?
    } else {
        Command::new("xdg-open").arg(&absolute).status()?
    };
    if !status.success() {
        bail!("default browser command failed");
    }
    Ok(())
}

fn safe_filename_label(value: &str) -> String {
    let value = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let value = value
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if value.is_empty() {
        "course".into()
    } else {
        value.chars().take(60).collect()
    }
}

#[derive(Deserialize)]
struct StudyIndexRequest {
    directory: PathBuf,
    out: Option<PathBuf>,
    course: Option<String>,
    instructor: Option<String>,
    #[serde(default)]
    include_speaker_notes: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_events_cli_exposes_exact_or_automatic_permission_modes() {
        let cli = Cli::try_parse_from([
            "inquiry",
            "live-events",
            "--approved-plan",
            "sha256:fixture",
        ])
        .unwrap();
        match cli.command {
            Commands::LiveEvents {
                offline,
                approved_plan,
                automatic_public_web,
            } => {
                assert!(!offline);
                assert_eq!(approved_plan.as_deref(), Some("sha256:fixture"));
                assert!(!automatic_public_web);
            }
            _ => panic!("expected live-events command"),
        }

        assert!(
            Cli::try_parse_from([
                "inquiry",
                "live-events",
                "--approved-plan",
                "sha256:fixture",
                "--automatic-public-web",
            ])
            .is_err()
        );
    }
}
