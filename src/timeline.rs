use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use url::Url;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

const MAX_EVENTS: usize = 250;
const MAX_FACTS_PER_EVENT: usize = 20;
const MAX_SOURCES_PER_EVENT: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineArtifact {
    #[serde(default = "schema_version")]
    pub schema_version: String,
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub notes: Vec<String>,
    pub events: Vec<TimelineEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    #[serde(default)]
    pub id: String,
    pub sort_key: i64,
    pub date_label: String,
    #[serde(default)]
    pub end_label: Option<String>,
    pub title: String,
    #[serde(default)]
    pub category: String,
    pub summary: String,
    #[serde(default)]
    pub facts: Vec<TimelineFact>,
    pub sources: Vec<TimelineSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineFact {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineSource {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
}

pub fn write_html(artifact: &TimelineArtifact, path: impl AsRef<Path>) -> Result<PathBuf> {
    validate(artifact)?;
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        if parent.exists() {
            let metadata = fs::symlink_metadata(parent).with_context(|| {
                format!("could not inspect timeline output {}", parent.display())
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                bail!("timeline output parent must be a real directory");
            }
        } else {
            fs::create_dir_all(parent).with_context(|| {
                format!("could not create timeline output {}", parent.display())
            })?;
        }
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path).with_context(|| {
        format!(
            "could not create {}; Inquiry never overwrites an existing timeline",
            path.display()
        )
    })?;
    file.write_all(render_html(artifact)?.as_bytes())?;
    Ok(path.to_path_buf())
}

pub fn render_html(artifact: &TimelineArtifact) -> Result<String> {
    validate(artifact)?;
    let mut events = artifact.events.iter().collect::<Vec<_>>();
    events.sort_by(|left, right| {
        left.sort_key
            .cmp(&right.sort_key)
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.date_label.cmp(&right.date_label))
    });
    let categories = events
        .iter()
        .filter_map(|event| {
            let category = event.category.trim();
            (!category.is_empty()).then_some(category)
        })
        .collect::<BTreeSet<_>>();
    let category_options = categories
        .iter()
        .map(|category| {
            format!(
                r#"<option value="{}">{}</option>"#,
                html_escape(&category.to_lowercase()),
                html_escape(category)
            )
        })
        .collect::<String>();
    let notes = artifact
        .notes
        .iter()
        .map(|note| format!("<li>{}</li>", html_escape(note)))
        .collect::<String>();
    let cards = events
        .iter()
        .enumerate()
        .map(|(position, event)| render_event(event, position))
        .collect::<Result<String>>()?;

    Ok(format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="color-scheme" content="dark light">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src 'none'; connect-src 'none'; font-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'">
<title>{title} — Inquiry Timeline</title>
<style>
:root{{--bg:#06100e;--panel:#0d1b18;--raised:#12231f;--ink:#f6f4ec;--muted:#9cafaa;--line:#29453e;--lime:#c8ff6b;--aqua:#65e6d4;--amber:#ffc76b;--shadow:0 24px 70px rgba(0,0,0,.28)}}
*{{box-sizing:border-box}}
html{{scroll-behavior:smooth}}
body{{margin:0;background:radial-gradient(circle at 75% -10%,rgba(101,230,212,.13),transparent 34rem),var(--bg);color:var(--ink);font:15px/1.55 Inter,ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}}
button,input,select{{font:inherit}}
a{{color:var(--aqua);text-underline-offset:3px}}
a:focus-visible,button:focus-visible,input:focus-visible,select:focus-visible{{outline:3px solid var(--lime);outline-offset:3px}}
.shell{{width:min(1180px,calc(100% - 28px));margin:28px auto 80px}}
.hero,.controls,.event,.notes{{border:1px solid var(--line);background:linear-gradient(145deg,var(--raised),var(--panel));border-radius:24px;box-shadow:var(--shadow)}}
.hero{{padding:clamp(24px,6vw,58px);position:relative;overflow:hidden}}
.eyebrow{{font:700 12px/1.2 ui-monospace,SFMono-Regular,Menlo,monospace;letter-spacing:.14em;text-transform:uppercase;color:var(--lime)}}
h1{{font-size:clamp(38px,8vw,82px);letter-spacing:-.055em;line-height:.95;margin:.22em 0}}
.subtitle{{max-width:72ch;color:var(--muted);font-size:clamp(16px,2vw,20px)}}
.proof{{display:flex;flex-wrap:wrap;gap:10px;margin-top:24px}}
.chip{{border:1px solid var(--line);border-radius:999px;padding:7px 11px;color:var(--muted);background:rgba(0,0,0,.16)}}
.controls{{position:sticky;top:10px;z-index:5;margin:16px 0;padding:14px;display:grid;grid-template-columns:minmax(180px,2fr) minmax(150px,1fr) auto auto;gap:10px;align-items:end}}
label{{display:grid;gap:5px;color:var(--muted);font-size:13px}}
input,select,button{{min-height:44px;border-radius:13px;border:1px solid var(--line);background:#091713;color:var(--ink);padding:10px 12px}}
button{{cursor:pointer;font-weight:750}}
button:hover{{border-color:var(--aqua)}}
.primary{{background:var(--lime);color:#10200b;border-color:transparent}}
.status{{grid-column:1/-1;color:var(--muted);min-height:1.4em}}
.timeline{{position:relative;display:grid;gap:16px;padding-left:34px}}
.timeline:before{{content:"";position:absolute;left:12px;top:4px;bottom:4px;width:2px;background:linear-gradient(var(--lime),var(--aqua),var(--line))}}
.event{{position:relative;padding:clamp(18px,4vw,30px)}}
.event:before{{content:"";position:absolute;left:-29px;top:31px;width:13px;height:13px;border-radius:50%;background:var(--lime);box-shadow:0 0 0 6px var(--bg)}}
.event-head{{display:grid;grid-template-columns:minmax(0,1fr) auto;gap:14px;align-items:start}}
.date{{color:var(--amber);font:700 13px/1.3 ui-monospace,SFMono-Regular,Menlo,monospace}}
h2{{font-size:clamp(23px,4vw,34px);line-height:1.08;margin:.28em 0}}
.category{{border:1px solid var(--line);border-radius:999px;padding:6px 10px;color:var(--aqua);font-size:12px;white-space:nowrap}}
.summary{{color:var(--muted);max-width:80ch}}
.facts{{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:9px;margin:18px 0}}
.fact{{border:1px solid var(--line);background:rgba(0,0,0,.13);border-radius:14px;padding:11px}}
.fact dt{{font-size:11px;color:var(--muted);text-transform:uppercase;letter-spacing:.08em}}
.fact dd{{margin:4px 0 0;font-weight:700}}
.sources{{margin-top:18px;border-top:1px solid var(--line);padding-top:14px}}
.sources h3{{font-size:14px;margin:0 0 8px}}
.sources ol{{margin:0;padding-left:22px}}
.sources li{{margin:7px 0;color:var(--muted)}}
.source-meta{{font-size:12px}}
.notes{{margin-top:18px;padding:22px}}
.notes li{{color:var(--muted);margin:6px 0}}
[hidden]{{display:none!important}}
.empty{{border:1px dashed var(--line);border-radius:20px;padding:34px;text-align:center;color:var(--muted)}}
@media(max-width:760px){{.controls{{position:static;grid-template-columns:1fr 1fr}}.controls label:first-child{{grid-column:1/-1}}.event-head{{grid-template-columns:1fr}}.category{{justify-self:start}}}}
@media(max-width:480px){{.controls{{grid-template-columns:1fr}}.controls label:first-child{{grid-column:auto}}.timeline{{padding-left:24px}}.timeline:before{{left:7px}}.event:before{{left:-23px}}}}
@media(prefers-reduced-motion:reduce){{html{{scroll-behavior:auto}}}}
@media(prefers-color-scheme:light){{:root{{--bg:#edf4f0;--panel:#fff;--raised:#fff;--ink:#11231e;--muted:#516961;--line:#bcd0c8;--lime:#568910;--aqua:#087d70;--amber:#9b5a00;--shadow:0 18px 55px rgba(29,58,48,.12)}}input,select,button{{background:#fff}}.primary{{color:#fff;background:#3d6f08}}}}
@media print{{.controls{{display:none}}body{{background:#fff}}.hero,.event,.notes{{box-shadow:none;break-inside:avoid}}}}
</style>
</head>
<body>
<main class="shell">
<header class="hero">
<div class="eyebrow">BarnLabs / Inquiry · source-backed interactive artifact</div>
<h1>{title}</h1>
<p class="subtitle">{subtitle}</p>
<div class="proof">
<span class="chip">{event_count} events</span>
<span class="chip">{source_count} citations</span>
<span class="chip">No remote media or data requests</span>
</div>
</header>
<section class="controls" aria-label="Timeline controls">
<label>Search events<input id="search" type="search" autocomplete="off" placeholder="Name, date, fact, topic…" aria-controls="timeline"></label>
<label>Category<select id="category" aria-controls="timeline"><option value="">All categories</option>{category_options}</select></label>
<button id="sort" type="button" aria-pressed="false">Newest first</button>
<button id="copy" class="primary" type="button">Copy visible citations</button>
<button id="csv" type="button">Download visible CSV</button>
<div id="status" class="status" role="status" aria-live="polite"></div>
</section>
<section id="timeline" class="timeline" aria-label="Timeline events">
{cards}
<div id="empty" class="empty" hidden>No events match the current filters.</div>
</section>
<aside class="notes"><h2>How to read this</h2><ul>{notes}<li>Every event needs at least one HTTPS citation. Source links open only when you choose them.</li><li>This artifact organizes supplied evidence; it does not independently establish that every supplied claim is correct.</li></ul></aside>
</main>
<script>
(()=>{{"use strict";
const q=document.getElementById("search"),cat=document.getElementById("category"),sort=document.getElementById("sort"),status=document.getElementById("status"),empty=document.getElementById("empty"),timeline=document.getElementById("timeline"),cards=[...document.querySelectorAll(".event")];
let descending=false;
const visible=()=>cards.filter(card=>!card.hidden);
const apply=()=>{{const needle=q.value.trim().toLocaleLowerCase(),category=cat.value;let shown=0;for(const card of cards){{const okText=!needle||card.dataset.search.includes(needle),okCat=!category||card.dataset.category===category;card.hidden=!(okText&&okCat);if(!card.hidden)shown++;}}empty.hidden=shown!==0;status.textContent=`${{shown}} of ${{cards.length}} events shown`;timeline.append(...cards.slice().sort((a,b)=>descending?Number(b.dataset.sort)-Number(a.dataset.sort):Number(a.dataset.sort)-Number(b.dataset.sort)));timeline.append(empty);}};
q.addEventListener("input",apply);cat.addEventListener("change",apply);
sort.addEventListener("click",()=>{{descending=!descending;sort.setAttribute("aria-pressed",String(descending));sort.textContent=descending?"Oldest first":"Newest first";apply();}});
const fallbackCopy=text=>{{const area=document.createElement("textarea");area.value=text;area.setAttribute("readonly","");area.style.position="fixed";area.style.opacity="0";document.body.append(area);area.select();const ok=document.execCommand("copy");area.remove();return ok;}};
document.getElementById("copy").addEventListener("click",async event=>{{const text=visible().flatMap(card=>[...card.querySelectorAll(".sources a")].map(link=>`${{card.querySelector("h2").textContent}} — ${{link.textContent}} — ${{link.href}}`)).join("\n");let ok=false;try{{await navigator.clipboard.writeText(text);ok=true;}}catch{{ok=fallbackCopy(text);}}event.currentTarget.textContent=ok?"Copied":"Copy unavailable";setTimeout(()=>event.currentTarget.textContent="Copy visible citations",1600);}});
const formulaSafe=value=>{{const visible=value.replace(/^[\\s\\uFEFF\\u200E\\u200F\\u202A-\\u202E\\u2066-\\u2069]+/u,"");return "=+-@".includes(visible[0])?"'"+value:value;}};
const csvCell=value=>`"${{formulaSafe(value).replaceAll('"','""')}}"`;
document.getElementById("csv").addEventListener("click",()=>{{const rows=[["Date","Title","Category","Summary","Source URLs"],...visible().map(card=>[card.dataset.date,card.querySelector("h2").textContent,card.dataset.categoryLabel,card.querySelector(".summary").textContent,[...card.querySelectorAll(".sources a")].map(a=>a.href).join(" ")])];const blob=new Blob([rows.map(row=>row.map(csvCell).join(",")).join("\\r\\n")],{{type:"text/csv;charset=utf-8"}}),url=URL.createObjectURL(blob),link=document.createElement("a");link.href=url;link.download="inquiry-timeline.csv";link.click();setTimeout(()=>URL.revokeObjectURL(url),0);}});
apply();
}})();
</script>
</body>
</html>"#,
        title = html_escape(&artifact.title),
        subtitle = html_escape(&artifact.subtitle),
        event_count = artifact.events.len(),
        source_count = artifact
            .events
            .iter()
            .map(|event| event.sources.len())
            .sum::<usize>(),
        category_options = category_options,
        cards = cards,
        notes = notes,
    ))
}

fn render_event(event: &TimelineEvent, position: usize) -> Result<String> {
    let category = event.category.trim();
    let category_badge = if category.is_empty() {
        String::new()
    } else {
        format!(r#"<span class="category">{}</span>"#, html_escape(category))
    };
    let facts = if event.facts.is_empty() {
        String::new()
    } else {
        format!(
            r#"<dl class="facts">{}</dl>"#,
            event
                .facts
                .iter()
                .map(|fact| format!(
                    r#"<div class="fact"><dt>{}</dt><dd>{}</dd></div>"#,
                    html_escape(&fact.label),
                    html_escape(&fact.value)
                ))
                .collect::<String>()
        )
    };
    let sources = event
        .sources
        .iter()
        .map(|source| {
            validate_source(source)?;
            let mut metadata = Vec::new();
            if let Some(publisher) = source.publisher.as_deref() {
                metadata.push(html_escape(publisher));
            }
            if let Some(date) = source.date.as_deref() {
                metadata.push(html_escape(date));
            }
            let metadata = if metadata.is_empty() {
                String::new()
            } else {
                format!(
                    r#" <span class="source-meta">({})</span>"#,
                    metadata.join(" · ")
                )
            };
            Ok(format!(
                r#"<li><a href="{}" target="_blank" rel="noopener noreferrer">{}</a>{}</li>"#,
                html_escape(&source.url),
                html_escape(&source.title),
                metadata
            ))
        })
        .collect::<Result<String>>()?;
    let end = event
        .end_label
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" – {}", html_escape(value)))
        .unwrap_or_default();
    let search_text = format!(
        "{} {} {} {} {}",
        event.date_label,
        event.title,
        event.category,
        event.summary,
        event
            .facts
            .iter()
            .map(|fact| format!("{} {}", fact.label, fact.value))
            .collect::<Vec<_>>()
            .join(" ")
    )
    .to_lowercase();
    let id = if event.id.trim().is_empty() {
        format!("event-{}", position + 1)
    } else {
        event.id.clone()
    };
    Ok(format!(
        r#"<article class="event" id="{id}" data-sort="{sort}" data-date="{date}" data-category="{category_value}" data-category-label="{category_label}" data-search="{search}">
<div class="event-head"><div><div class="date">{date}{end}</div><h2>{title}</h2></div>{category_badge}</div>
<p class="summary">{summary}</p>
{facts}
<div class="sources"><h3>Sources</h3><ol>{sources}</ol></div>
</article>"#,
        id = html_escape(&id),
        sort = event.sort_key,
        date = html_escape(&event.date_label),
        end = end,
        category_value = html_escape(&category.to_lowercase()),
        category_label = html_escape(category),
        search = html_escape(&search_text),
        title = html_escape(&event.title),
        category_badge = category_badge,
        summary = html_escape(&event.summary),
        facts = facts,
        sources = sources,
    ))
}

fn validate(artifact: &TimelineArtifact) -> Result<()> {
    if artifact.schema_version != schema_version() {
        bail!("unsupported timeline schema");
    }
    bounded(&artifact.title, "timeline title", 1, 200)?;
    bounded(&artifact.subtitle, "timeline subtitle", 0, 600)?;
    if artifact.events.is_empty() || artifact.events.len() > MAX_EVENTS {
        bail!("timeline must contain 1 to {MAX_EVENTS} events");
    }
    if artifact.notes.len() > 20 {
        bail!("timeline cannot contain more than 20 notes");
    }
    for note in &artifact.notes {
        bounded(note, "timeline note", 1, 500)?;
    }
    let mut ids = HashSet::new();
    for event in &artifact.events {
        bounded(&event.id, "event id", 0, 100)?;
        if !event.id.is_empty()
            && (!event.id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            }) || !ids.insert(event.id.as_str()))
        {
            bail!("timeline event IDs must be unique ASCII slugs");
        }
        bounded(&event.date_label, "event date label", 1, 100)?;
        if let Some(end) = event.end_label.as_deref() {
            bounded(end, "event end label", 1, 100)?;
        }
        bounded(&event.title, "event title", 1, 240)?;
        bounded(&event.category, "event category", 0, 100)?;
        bounded(&event.summary, "event summary", 1, 2_000)?;
        if event.facts.len() > MAX_FACTS_PER_EVENT {
            bail!("a timeline event exceeds the {MAX_FACTS_PER_EVENT}-fact limit");
        }
        for fact in &event.facts {
            bounded(&fact.label, "fact label", 1, 100)?;
            bounded(&fact.value, "fact value", 1, 500)?;
        }
        if event.sources.is_empty() || event.sources.len() > MAX_SOURCES_PER_EVENT {
            bail!("every timeline event needs 1 to {MAX_SOURCES_PER_EVENT} HTTPS source citations");
        }
        for source in &event.sources {
            validate_source(source)?;
        }
    }
    Ok(())
}

fn validate_source(source: &TimelineSource) -> Result<()> {
    bounded(&source.title, "source title", 1, 300)?;
    bounded(&source.url, "source URL", 1, 2_000)?;
    if let Some(publisher) = source.publisher.as_deref() {
        bounded(publisher, "source publisher", 1, 200)?;
    }
    if let Some(date) = source.date.as_deref() {
        bounded(date, "source date", 1, 100)?;
    }
    let url = Url::parse(&source.url).context("timeline source URL is invalid")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        bail!("timeline sources must use credential-free HTTPS URLs");
    }
    Ok(())
}

fn bounded(value: &str, field: &str, minimum: usize, maximum: usize) -> Result<()> {
    let length = value.trim().chars().count();
    if length < minimum || length > maximum {
        bail!("{field} must contain {minimum} to {maximum} characters");
    }
    Ok(())
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
        .chars()
        .map(|character| {
            if character.is_control() && !matches!(character, '\n' | '\t') {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn schema_version() -> String {
    "inquiry.timeline/v1".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fixture() -> TimelineArtifact {
        TimelineArtifact {
            schema_version: schema_version(),
            title: "Verified presidents timeline".into(),
            subtitle: "A compact source-backed example.".into(),
            notes: vec!["Dates and claims require source review.".into()],
            events: vec![TimelineEvent {
                id: "example".into(),
                sort_key: 17890430,
                date_label: "1789-04-30".into(),
                end_label: None,
                title: "Example <script>alert(1)</script>".into(),
                category: "Presidency".into(),
                summary: "A supplied summary, rendered only as text.".into(),
                facts: vec![TimelineFact {
                    label: "Ordinal".into(),
                    value: "1".into(),
                }],
                sources: vec![TimelineSource {
                    title: "Official source".into(),
                    url: "https://example.gov/source".into(),
                    publisher: Some("Example.gov".into()),
                    date: Some("2026-07-16".into()),
                }],
            }],
        }
    }

    #[test]
    fn renders_self_contained_interactive_timeline_without_active_input() {
        let html = render_html(&fixture()).unwrap();
        assert!(html.contains("Content-Security-Policy"));
        assert!(html.contains("Download visible CSV"));
        assert!(html.contains("Copy visible citations"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(!html.contains("<img"));
        assert!(!html.contains("https://cdn"));
    }

    #[test]
    fn rejects_non_https_and_credentialed_sources() {
        let mut artifact = fixture();
        artifact.events[0].sources[0].url = "http://example.com".into();
        assert!(render_html(&artifact).is_err());
        artifact.events[0].sources[0].url = "https://user:secret@example.com".into();
        assert!(render_html(&artifact).is_err());
    }

    #[test]
    fn refuses_overwrite() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("timeline.html");
        write_html(&fixture(), &path).unwrap();
        assert!(write_html(&fixture(), &path).is_err());
    }
}
