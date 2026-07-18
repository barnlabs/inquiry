use crate::model::{ContentTrust, ResearchReport, SourceQuality};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const REPORT_STYLES: &str = r#"
:root {
  color-scheme: dark light;
  --canvas: #0a0f0e;
  --paper: #101615;
  --paper-soft: #151c1a;
  --ink: #f2efe6;
  --muted: #a4ada8;
  --faint: #737e79;
  --rule: #36423e;
  --rule-strong: #596661;
  --accent: #b9ef78;
  --link: #77ddd2;
  --caution: #f0c975;
  --danger: #ff8b80;
  --serif: ui-serif, Charter, "Iowan Old Style", "Palatino Linotype", Georgia, serif;
  --sans: ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  --mono: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

* { box-sizing: border-box; }
html { scroll-behavior: smooth; }
body {
  margin: 0;
  overflow-x: hidden;
  background: var(--canvas);
  color: var(--ink);
  font: 15px/1.62 var(--sans);
  text-rendering: optimizeLegibility;
}
a { color: var(--link); text-decoration-thickness: .08em; text-underline-offset: .16em; }
a:hover { text-decoration-thickness: .14em; }
button, input { font: inherit; }
button:focus-visible, input:focus-visible, a:focus-visible, [tabindex]:focus-visible {
  outline: 3px solid var(--link);
  outline-offset: 3px;
}
.shell { width: min(1080px, calc(100% - 40px)); margin-inline: auto; }
.masthead { border-bottom: 1px solid var(--rule-strong); background: var(--paper); }
.topbar {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 24px;
  padding: 22px 0;
  border-bottom: 1px solid var(--rule);
}
.wordmark { margin: 0; font-size: 19px; font-weight: 760; letter-spacing: -.02em; }
.wordmark small { margin-left: 8px; color: var(--muted); font-size: 11px; font-weight: 620; letter-spacing: .12em; text-transform: uppercase; }
.run-id { margin: 0; color: var(--faint); font: 11px/1.4 var(--mono); overflow-wrap: anywhere; }
.hero {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(320px, 400px);
  gap: clamp(36px, 6vw, 72px);
  align-items: end;
  padding: clamp(44px, 8vw, 86px) 0 clamp(42px, 7vw, 70px);
}
.kicker, .section-label {
  margin: 0;
  color: var(--accent);
  font: 700 11px/1.4 var(--mono);
  letter-spacing: .14em;
  text-transform: uppercase;
}
h1 {
  max-width: 850px;
  margin: 12px 0 20px;
  font: 600 clamp(38px, 6vw, 66px)/1.02 var(--serif);
  letter-spacing: -.04em;
  text-wrap: balance;
}
.lede { max-width: 760px; margin: 0; color: #c8cfcb; font-size: clamp(16px, 2vw, 19px); }
.hero-meta { margin: 18px 0 0; color: var(--faint); font: 11px/1.5 var(--mono); }
.assessment { padding-left: 24px; border-left: 1px solid var(--rule-strong); }
.assessment-status { margin: 0 0 5px; color: var(--accent); font: 700 10px/1.4 var(--mono); letter-spacing: .12em; text-transform: uppercase; }
.assessment h2 { margin: 0; font: 600 25px/1.15 var(--serif); letter-spacing: -.02em; }
.assessment-explanation { margin: 9px 0 20px; color: var(--muted); font-size: 12px; line-height: 1.5; }
.evidence-dimensions { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px 18px; margin: 0; }
.evidence-dimensions div { padding-top: 9px; border-top: 1px solid var(--rule); }
.evidence-dimensions dt { color: var(--faint); font: 700 9px/1.35 var(--mono); letter-spacing: .06em; text-transform: uppercase; }
.evidence-dimensions dd { margin: 3px 0 0; font-size: 12px; font-weight: 700; }
.evidence-dimensions dd.strong { color: var(--accent); }
.evidence-dimensions dd.moderate { color: var(--link); }
.evidence-dimensions dd.limited { color: var(--caution); }
.evidence-dimensions dd.not-applicable { color: var(--faint); }
.evidence-dimensions small { display: block; margin-top: 2px; color: var(--faint); font-size: 9px; font-weight: 500; }
main { padding-bottom: 64px; }
.dossier-section { padding: clamp(34px, 6vw, 62px) 0; border-bottom: 1px solid var(--rule-strong); }
.section-head {
  display: grid;
  grid-template-columns: minmax(120px, 170px) minmax(0, 1fr);
  gap: 28px;
  align-items: start;
  margin-bottom: 28px;
}
.section-head h2 { margin: -4px 0 6px; font: 600 clamp(25px, 3.6vw, 36px)/1.12 var(--serif); letter-spacing: -.025em; }
.section-head p { max-width: 700px; margin: 0; color: var(--muted); }
.research-rationale { max-width: 820px; margin: 0 0 30px 198px; color: #c9d0cc; }
.report-tools {
  display: grid;
  grid-template-columns: minmax(240px, .8fr) minmax(0, 1.2fr);
  gap: 24px 34px;
  align-items: end;
  padding: 22px 0;
  border-top: 1px solid var(--rule);
  border-bottom: 1px solid var(--rule);
}
.search-control label, .facet-label { display: block; margin-bottom: 8px; color: var(--muted); font-size: 12px; font-weight: 650; }
.search-control input {
  width: 100%;
  min-height: 44px;
  border: 0;
  border-bottom: 1px solid var(--rule-strong);
  border-radius: 0;
  background: transparent;
  color: var(--ink);
  padding: 8px 2px;
  appearance: none;
}
.search-control input::placeholder { color: var(--faint); }
.filters { display: flex; flex-wrap: wrap; gap: 2px 18px; }
.filter {
  min-height: 44px;
  padding: 8px 0 6px;
  border: 0;
  border-bottom: 2px solid transparent;
  border-radius: 0;
  background: transparent;
  color: var(--muted);
  cursor: pointer;
}
.filter:hover { color: var(--ink); }
.filter.active { border-bottom-color: var(--accent); color: var(--accent); }
.filter-status { grid-column: 1 / -1; min-height: 1.4em; margin: -8px 0 0; color: var(--faint); font-size: 12px; }
.metrics { border-top: 1px solid var(--rule); }
.metric {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(180px, 34%);
  gap: 22px;
  align-items: center;
  padding: 20px 0;
  border-bottom: 1px solid var(--rule);
}
.metric-head { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 18px; align-items: baseline; }
.metric-head span { font-weight: 650; }
.metric-head strong { color: var(--accent); font: 600 21px/1.2 var(--serif); text-align: right; }
.metric-head small { color: var(--muted); font: 500 12px/1.2 var(--sans); }
.metric-detail p { margin: 0; color: var(--muted); font-size: 12px; }
.bar { height: 3px; margin: 9px 0 7px; overflow: hidden; background: var(--rule); }
.bar i { display: block; height: 100%; background: var(--accent); }
.findings { border-top: 1px solid var(--rule-strong); }
.finding {
  display: grid;
  grid-template-columns: 56px minmax(0, 1fr);
  gap: 22px;
  padding: clamp(28px, 5vw, 48px) 0;
  border-bottom: 1px solid var(--rule);
}
.entry-number { color: var(--faint); font: 500 26px/1 var(--serif); }
.entry-meta { display: flex; flex-wrap: wrap; gap: 7px 18px; margin: 0 0 8px; color: var(--muted); font: 11px/1.4 var(--mono); text-transform: uppercase; }
.entry-meta .high { color: var(--accent); }
.entry-meta .moderate { color: var(--caution); }
.entry-meta .low { color: var(--danger); }
.finding h3 { max-width: 800px; margin: 0 0 13px; font: 600 clamp(21px, 3vw, 29px)/1.2 var(--serif); letter-spacing: -.02em; }
.finding-body { max-width: 800px; margin: 0; color: #c8cfcb; font-size: 16px; }
.tags { display: flex; flex-wrap: wrap; gap: 4px 18px; margin: 17px 0 0; padding: 0; color: var(--muted); font: 11px/1.5 var(--mono); list-style: none; }
.tags li + li::before { content: "·"; margin-right: 18px; color: var(--faint); }
.media-note { max-width: 760px; margin: 20px 0; padding: 2px 0 2px 15px; border-left: 2px solid var(--link); }
.media-note strong { display: block; color: var(--link); }
.media-note span { display: block; margin-top: 3px; color: var(--muted); font-size: 12px; }
.entry-sources { margin-top: 22px; padding-top: 14px; border-top: 1px solid var(--rule); color: var(--muted); font-size: 12px; }
.entry-sources > span:first-child { margin-right: 12px; color: var(--faint); font: 700 10px/1.4 var(--mono); letter-spacing: .12em; text-transform: uppercase; }
.notice { max-width: 850px; padding-left: 18px; border-left: 3px solid var(--caution); }
.warnings { margin: 0; padding-left: 20px; color: #ded3b8; }
.warnings li + li { margin-top: 8px; }
.quiet { margin: 0; color: var(--muted); }
.ledger-actions { display: flex; align-items: center; gap: 12px; margin: 0 0 14px 198px; }
.copy {
  min-height: 44px;
  padding: 7px 0;
  border: 0;
  border-bottom: 1px solid var(--link);
  border-radius: 0;
  background: transparent;
  color: var(--link);
  cursor: pointer;
}
.copy-status { color: var(--muted); font-size: 12px; }
.table-wrap { max-width: 100%; overflow-x: auto; -webkit-overflow-scrolling: touch; }
table { width: 100%; border-collapse: collapse; }
.visually-hidden { position: absolute !important; width: 1px !important; height: 1px !important; padding: 0 !important; margin: -1px !important; overflow: hidden !important; clip: rect(0 0 0 0) !important; white-space: nowrap !important; border: 0 !important; }
th, td { padding: 15px 12px; border-bottom: 1px solid var(--rule); text-align: left; vertical-align: top; }
th:first-child, td:first-child { padding-left: 0; }
th:last-child, td:last-child { padding-right: 0; }
th { color: var(--faint); font: 700 10px/1.4 var(--mono); letter-spacing: .1em; text-transform: uppercase; }
td { overflow-wrap: anywhere; }
td small { display: block; margin-top: 4px; color: var(--muted); line-height: 1.45; }
.source-title { font-weight: 650; }
.data-table-artifact + .data-table-artifact { margin-top: 36px; }
.data-table-artifact h3 { margin: 0 0 7px; font: 600 22px/1.2 var(--serif); }
.data-table-artifact > p { max-width: 820px; color: var(--muted); }
.data-table-notes { margin: 16px 0 0; padding-left: 20px; color: var(--muted); font-size: 12px; }
.tier { font-size: 11px; }
.tier.primary { color: var(--accent); }
.tier.strong-secondary { color: var(--link); }
.tier.discovery { color: var(--caution); }
.provenance {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin: 0;
  border-top: 1px solid var(--rule);
}
.provenance div { min-width: 0; padding: 16px 18px 16px 0; border-bottom: 1px solid var(--rule); }
.provenance dt { color: var(--faint); font: 700 10px/1.4 var(--mono); letter-spacing: .1em; text-transform: uppercase; }
.provenance dd { margin: 5px 0 0; overflow-wrap: anywhere; font: 12px/1.5 var(--mono); }
.connector-errors { max-width: 850px; margin-top: 28px; }
.connector-errors h3 { margin: 0 0 9px; font: 600 18px/1.3 var(--serif); }
.empty-state { margin: 24px 0 0 78px; color: var(--muted); font-style: italic; }
[hidden] { display: none !important; }
.page-footer { display: flex; justify-content: space-between; gap: 24px; padding: 26px 0 50px; color: var(--faint); font-size: 12px; }

@media (max-width: 760px) {
  .shell { width: min(100% - 24px, 1080px); }
  .topbar { align-items: flex-start; flex-direction: column; gap: 8px; }
  .hero { grid-template-columns: 1fr; gap: 30px; }
  .assessment { padding: 20px 0 0; border-top: 1px solid var(--rule-strong); border-left: 0; }
  .section-head, .report-tools, .metric { grid-template-columns: 1fr; }
  .section-head { gap: 10px; }
  .research-rationale, .ledger-actions { margin-left: 0; }
  .finding { grid-template-columns: 38px minmax(0, 1fr); gap: 12px; }
  .entry-meta { gap: 6px 12px; }
  .empty-state { margin-left: 50px; }
  .provenance { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .table-wrap { overflow: visible; }
  table, thead, tbody, tr, th, td { display: block; width: 100%; }
  thead { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
  tbody { border-top: 1px solid var(--rule-strong); }
  tr { padding: 16px 0; border-bottom: 1px solid var(--rule); }
  td, td:first-child, td:last-child { display: grid; grid-template-columns: minmax(88px, 30%) minmax(0, 1fr); gap: 12px; padding: 6px 0; border: 0; }
  td::before { content: attr(data-label); color: var(--faint); font: 700 10px/1.4 var(--mono); letter-spacing: .08em; text-transform: uppercase; }
  .page-footer { flex-direction: column; gap: 5px; }
}

@media (max-width: 420px) {
  .evidence-dimensions { grid-template-columns: 1fr; }
  .provenance { grid-template-columns: 1fr; }
  .finding { grid-template-columns: 1fr; }
  .entry-number { font-size: 18px; }
  .empty-state { margin-left: 0; }
}

@media (prefers-reduced-motion: reduce) {
  html { scroll-behavior: auto; }
}

@media (prefers-color-scheme: light) {
  :root {
    --canvas: #f5f3ed;
    --paper: #fbfaf6;
    --paper-soft: #f0eee7;
    --ink: #18211e;
    --muted: #58645f;
    --faint: #737d79;
    --rule: #cbd0cc;
    --rule-strong: #9fa9a4;
    --accent: #426f18;
    --link: #087d72;
    --caution: #765414;
    --danger: #a22c26;
  }
  .lede, .finding-body, .research-rationale { color: #394740; }
  .warnings { color: #5f4a1f; }
}

@page { margin: 16mm; }
@media print {
  :root { --canvas: #fff; --paper: #fff; --ink: #111; --muted: #444; --faint: #555; --rule: #bbb; --rule-strong: #777; --accent: #111; --link: #0645ad; --caution: #333; --danger: #111; }
  html { font-size: 10pt; }
  body { background: #fff; color: #111; }
  .shell { width: 100%; }
  .masthead { background: #fff; }
  .topbar { padding-top: 0; }
  .hero { display: block; padding: 28px 0; }
  h1 { font-size: 31pt; }
  .assessment { margin-top: 22px; padding: 14px 0 0; border-top: 1px solid #999; border-left: 0; }
  .evidence-dimensions { grid-template-columns: repeat(5, minmax(0, 1fr)); }
  .controls-section, .copy, .copy-status { display: none !important; }
  .dossier-section { padding: 24px 0; break-inside: auto; }
  .finding, .metric, .notice, .provenance div, tr { break-inside: avoid; }
  [hidden] { display: revert !important; }
  .source-ledger a[href]::after { content: " (" attr(href) ")"; color: #444; font-size: 8pt; font-weight: 400; }
  .media-note { border-color: #777; }
  .page-footer { padding-bottom: 0; }
}
"#;

const REPORT_SCRIPT: &str = r#"(() => {
  const search = document.getElementById('reportSearch');
  const filters = [...document.querySelectorAll('.filter')];
  const facetItems = [...document.querySelectorAll('[data-facet]')];
  const findings = [...document.querySelectorAll('[data-search-item="finding"]')];
  const tableRows = [...document.querySelectorAll('[data-search-item="table-row"]')];
  const dataTables = [...document.querySelectorAll('[data-table-artifact]')];
  const sourceRows = [...document.querySelectorAll('[data-search-item="source"]')];
  const status = document.getElementById('filterStatus');
  const findingsEmpty = document.getElementById('findingsEmpty');
  const tablesEmpty = document.getElementById('tablesEmpty');
  const sourcesEmpty = document.getElementById('sourcesEmpty');
  let activeFacet = 'all';

  const normalized = value => value.toLocaleLowerCase();
  const matchesSearch = (element, term) => !term || normalized(element.textContent || '').includes(term);
  const update = () => {
    const term = normalized(search.value.trim());
    facetItems.forEach(item => {
      const facetMatch = activeFacet === 'all' || item.dataset.facet === activeFacet;
      const textMatch = item.dataset.searchItem === 'finding' ? matchesSearch(item, term) : true;
      item.hidden = !(facetMatch && textMatch);
    });
    sourceRows.forEach(row => { row.hidden = !matchesSearch(row, term); });
    tableRows.forEach(row => { row.hidden = !matchesSearch(row, term); });
    dataTables.forEach(table => {
      const rows = [...table.querySelectorAll('[data-search-item="table-row"]')];
      table.hidden = rows.length > 0 && rows.every(row => row.hidden);
    });
    const visibleFindings = findings.filter(item => !item.hidden).length;
    const visibleTableRows = tableRows.filter(item => !item.hidden).length;
    const visibleSources = sourceRows.filter(item => !item.hidden).length;
    findingsEmpty.hidden = visibleFindings !== 0;
    if (tablesEmpty) { tablesEmpty.hidden = visibleTableRows !== 0; }
    sourcesEmpty.hidden = visibleSources !== 0;
    status.textContent = `Showing ${visibleFindings} of ${findings.length} findings, ${visibleTableRows} of ${tableRows.length} table rows, and ${visibleSources} of ${sourceRows.length} sources.`;
  };

  filters.forEach(button => button.addEventListener('click', () => {
    activeFacet = button.dataset.filter || 'all';
    filters.forEach(candidate => {
      const selected = candidate === button;
      candidate.classList.toggle('active', selected);
      candidate.setAttribute('aria-pressed', String(selected));
    });
    update();
  }));
  search.addEventListener('input', update);
  search.addEventListener('keydown', event => {
    if (event.key === 'Escape' && search.value) {
      search.value = '';
      update();
    }
  });
  document.addEventListener('keydown', event => {
    if (event.key === '/' && !event.metaKey && !event.ctrlKey && !event.altKey && !/^(INPUT|TEXTAREA|SELECT)$/.test(event.target.tagName)) {
      event.preventDefault();
      search.focus();
    }
  });

  const copyButton = document.getElementById('copySources');
  const copyStatus = document.getElementById('copyStatus');
  copyButton.addEventListener('click', async () => {
    const citations = sourceRows.map(row => {
      const link = row.querySelector('a');
      return link ? `${link.textContent} — ${link.href}` : row.cells[0].innerText.trim();
    }).join('\n');
    let copied = false;
    try {
      await navigator.clipboard.writeText(citations);
      copied = true;
    } catch {
      const area = document.createElement('textarea');
      area.value = citations;
      area.setAttribute('readonly', '');
      area.style.position = 'fixed';
      area.style.opacity = '0';
      document.body.appendChild(area);
      area.select();
      copied = document.execCommand('copy');
      area.remove();
    }
    copyStatus.textContent = copied ? 'Citations copied.' : 'Copy unavailable.';
    window.setTimeout(() => { copyStatus.textContent = ''; }, 1800);
  });

  update();
})();"#;

pub fn write_json(report: &ResearchReport, path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_new(path, &serde_json::to_vec_pretty(report)?)?;
    Ok(path.to_path_buf())
}

pub fn write_html(report: &ResearchReport, path: impl AsRef<Path>) -> Result<PathBuf> {
    validate_report(report)?;
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_new(path, render_html(report).as_bytes())?;
    Ok(path.to_path_buf())
}

fn write_new(path: &Path, content: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| {
            format!(
                "could not create {}; Inquiry never overwrites an existing report",
                path.display()
            )
        })?;
    file.write_all(content)
        .with_context(|| format!("could not write {}", path.display()))
}

pub fn render_html(report: &ResearchReport) -> String {
    let facets = report
        .plan
        .facets
        .iter()
        .map(|facet| {
            let value = facet.to_string();
            format!(
                r#"<button class="filter" data-filter="{}" aria-pressed="false">{}</button>"#,
                attr(&value),
                escape(&title_case(&value))
            )
        })
        .collect::<String>();
    let mut warning_items = report.warnings.clone();
    let rejected_links = report
        .sources
        .iter()
        .filter(|source| safe_http_url(&source.url).is_none())
        .count();
    if rejected_links > 0 {
        warning_items.push(format!(
            "{rejected_links} source link(s) used a non-HTTP(S) or invalid URL and were rendered as non-clickable text."
        ));
    }
    let warning_content = if warning_items.is_empty() {
        r#"<p class="quiet">No run warnings were recorded. Source limitations still apply.</p>"#
            .to_string()
    } else {
        let items = warning_items
            .iter()
            .map(|warning| format!("<li>{}</li>", escape(warning)))
            .collect::<String>();
        format!(r#"<div class="notice"><ul class="warnings">{items}</ul></div>"#)
    };
    let mut unit_max: HashMap<&str, f64> = HashMap::new();
    let mut unit_count: HashMap<&str, usize> = HashMap::new();
    for metric in &report.metrics {
        unit_max
            .entry(&metric.unit)
            .and_modify(|maximum| *maximum = maximum.max(metric.value.abs()))
            .or_insert(metric.value.abs());
        *unit_count.entry(&metric.unit).or_default() += 1;
    }
    let metrics = report
        .metrics
        .iter()
        .map(|metric| {
            let comparable = unit_count
                .get(metric.unit.as_str())
                .copied()
                .unwrap_or_default()
                > 1;
            let maximum = unit_max
                .get(metric.unit.as_str())
                .copied()
                .unwrap_or_default();
            let width = if comparable && maximum > 0.0 {
                (metric.value.abs() / maximum * 100.0).max(3.0)
            } else {
                0.0
            };
            let bar = if comparable {
                format!(
                    r#"<div class="bar" role="img" aria-label="Relative magnitude among metrics measured in {unit}"><i style="width:{width:.2}%"></i></div>"#,
                    unit = attr(&metric.unit),
                    width = width
                )
            } else {
                String::new()
            };
            let citations = metric
                .source_ids
                .iter()
                .filter_map(|id| report.sources.iter().find(|source| &source.id == id))
                .map(|source| source_link(source, "source"))
                .collect::<Vec<_>>()
                .join(" · ");
            let facet = metric.facet.to_string();
            format!(
                r#"<article class="metric" data-facet="{facet}"><div class="metric-head"><span>{label}</span><strong>{value} <small>{unit}</small></strong></div><div class="metric-detail">{bar}<p>{period}{separator}{citations}</p></div></article>"#,
                facet = attr(&facet),
                label = escape(&metric.label),
                value = escape(&metric.display_value),
                unit = escape(&metric.unit),
                bar = bar,
                period = escape(metric.period.as_deref().unwrap_or("period not supplied")),
                separator = if citations.is_empty() { "" } else { " · " },
                citations = citations
            )
        })
        .collect::<String>();
    let findings = report
        .findings
        .iter()
        .enumerate()
        .map(|(index, finding)| {
            let tags = if finding.tags.is_empty() {
                String::new()
            } else {
                let items = finding
                    .tags
                    .iter()
                    .map(|tag| format!("<li>{}</li>", escape(tag)))
                    .collect::<String>();
                format!(r#"<ul class="tags" aria-label="Finding tags">{items}</ul>"#)
            };
            let trust = match finding.content_trust {
                ContentTrust::CuratedTemplate => "curated template",
                ContentTrust::ExternalUntrusted => "untrusted excerpt",
            };
            let linked_sources = finding
                .source_ids
                .iter()
                .filter_map(|id| report.sources.iter().find(|source| &source.id == id))
                .collect::<Vec<_>>();
            let citations = linked_sources
                .iter()
                .map(|source| source_link(source, &source.publisher))
                .collect::<Vec<_>>()
                .join(" · ");
            let attachments = linked_sources
                .iter()
                .flat_map(|source| {
                    [
                        source
                            .provenance
                            .content_url
                            .as_deref()
                            .map(|url| external_link(url, "open file")),
                        source
                            .provenance
                            .preview_url
                            .as_deref()
                            .map(|url| external_link(url, "open preview")),
                    ]
                })
                .flatten()
                .collect::<Vec<_>>()
                .join(" · ");
            let media = linked_sources
                .iter()
                .find_map(|source| {
                    let url = source.provenance.preview_url.as_deref()?;
                    if !safe_preview_link_url(url) {
                        return None;
                    }
                    let parsed = url::Url::parse(url).ok()?;
                    let host = parsed.host_str()?;
                    Some(format!(
                        r#"<a class="media-note" href="{href}" target="_blank" rel="noopener noreferrer"><strong>Remote image link</strong><span>Click to contact {host} and inspect the image. Inquiry does not load remote media when this report opens. Verify identity and file-specific terms in the source ledger.</span></a>"#,
                        href = attr(url),
                        host = escape(host)
                    ))
                })
                .unwrap_or_default();
            let facet = finding.facet.to_string();
            let confidence = finding.confidence.to_string();
            format!(
                r#"<article class="finding" data-facet="{facet}" data-search-item="finding"><div class="entry-number" aria-hidden="true">{number:02}</div><div><p class="entry-meta"><span>{facet_label}</span><span>{trust}</span><span class="{confidence}">support {confidence}</span></p><h3>{title}</h3>{media}<p class="finding-body">{body}</p>{tags}<footer class="entry-sources"><span>Sources</span>{citations}{attachment_separator}{attachments}</footer></div></article>"#,
                facet = attr(&facet),
                facet_label = escape(&title_case(&facet)),
                number = index + 1,
                trust = trust,
                confidence = attr(&confidence),
                title = escape(&finding.title),
                media = media,
                body = escape(&finding.body),
                tags = tags,
                citations = if citations.is_empty() {
                    "Source record unavailable".to_string()
                } else {
                    citations
                },
                attachment_separator = if attachments.is_empty() { "" } else { " · " },
                attachments = attachments
            )
        })
        .collect::<String>();
    let sources = report
        .sources
        .iter()
        .map(|source| {
            let tier = match source.quality {
                SourceQuality::Primary => "primary",
                SourceQuality::StrongSecondary => "strong secondary",
                SourceQuality::DiscoveryOnly => "discovery",
            };
            let dataset = source
                .provenance
                .dataset_id
                .as_deref()
                .map(|value| format!(" · dataset {}", escape(value)))
                .unwrap_or_default();
            let observation = source
                .provenance
                .observation_period
                .as_deref()
                .map(|value| format!("<small>Observation {}</small>", escape(value)))
                .unwrap_or_default();
            let provenance_time = provenance_time_label(source);
            let attachments = [
                source
                    .provenance
                    .content_url
                    .as_deref()
                    .map(|url| external_link(url, "file")),
                source
                    .provenance
                    .preview_url
                    .as_deref()
                    .map(|url| external_link(url, "preview")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
            let attachments = if attachments.is_empty() {
                String::new()
            } else {
                format!("<small>{attachments}</small>")
            };
            format!(
                r#"<tr data-search-item="source"><td data-label="Source"><div class="source-title">{source_link}</div><small>{publisher} · ID {source_id}{dataset}</small>{attachments}</td><td data-label="Provenance time">{provenance_time}{observation}</td><td data-label="Tier"><span class="tier {tier_class}">{tier}</span></td><td data-label="License / terms">{license}</td></tr>"#,
                source_link = source_link(source, &source.title),
                publisher = escape(&source.publisher),
                source_id = escape(&source.id),
                dataset = dataset,
                attachments = attachments,
                provenance_time = escape(&provenance_time),
                observation = observation,
                tier_class = attr(&tier.replace(' ', "-")),
                tier = tier,
                license = escape(source.license.as_deref().unwrap_or("Check source terms"))
            )
        })
        .collect::<String>();
    let data_tables = report
        .tables
        .iter()
        .map(|table| {
            let headers = table
                .columns
                .iter()
                .map(|column| {
                    let label = table_column_label(column);
                    format!(r#"<th scope="col">{}</th>"#, escape(&label))
                })
                .collect::<String>();
            let rows = table
                .rows
                .iter()
                .map(|row| {
                    let cells = row
                        .cells
                        .iter()
                        .zip(&table.columns)
                        .map(|(cell, column)| {
                            let label = table_column_label(column);
                            format!(
                                r#"<td data-label="{}">{}</td>"#,
                                attr(&label),
                                escape(cell)
                            )
                        })
                        .collect::<String>();
                    format!(
                        r#"<tr data-search-item="table-row" data-row-id="{}">{cells}</tr>"#,
                        attr(&row.id)
                    )
                })
                .collect::<String>();
            let notes = if table.notes.is_empty() {
                String::new()
            } else {
                format!(
                    r#"<ul class="data-table-notes">{}</ul>"#,
                    table
                        .notes
                        .iter()
                        .map(|note| format!("<li>{}</li>", escape(note)))
                        .collect::<String>()
                )
            };
            let citations = table
                .source_ids
                .iter()
                .filter_map(|id| report.sources.iter().find(|source| &source.id == id))
                .map(|source| source_link(source, &source.publisher))
                .collect::<Vec<_>>()
                .join(" · ");
            let citations = if citations.is_empty() {
                String::new()
            } else {
                format!(r#"<p class="data-table-sources">Sources: {citations}</p>"#)
            };
            format!(
                r#"<article class="data-table-artifact" data-table-artifact><h3>{title}</h3><p>{description}</p><div class="table-wrap" tabindex="0" aria-label="{title} searchable table"><table><caption class="visually-hidden">{title}</caption><thead><tr>{headers}</tr></thead><tbody>{rows}</tbody></table></div>{notes}{citations}</article>"#,
                title = escape(&table.title),
                description = escape(&table.description),
            )
        })
        .collect::<String>();
    let table_content = if data_tables.is_empty() {
        String::new()
    } else {
        let empty_state = if report.tables.iter().all(|table| table.rows.is_empty()) {
            ""
        } else {
            "hidden"
        };
        format!(
            r#"<section class="dossier-section" aria-labelledby="tables-heading"><div class="section-head"><p class="section-label">Reference data</p><div><h2 id="tables-heading">Searchable tables</h2><p>Filter the full rows with the dossier search. Units, notes, and reviewed source pointers stay attached.</p></div></div><div class="data-tables">{data_tables}</div><p class="empty-state" id="tablesEmpty" {empty_state}>No table rows match the current search.</p></section>"#
        )
    };
    let connector_errors = report
        .run
        .connector_errors
        .iter()
        .map(|error| format!("<li>{}</li>", escape(error)))
        .collect::<String>();
    let attempted = report
        .run
        .connectors_attempted
        .iter()
        .map(|name| escape(name))
        .collect::<Vec<_>>()
        .join(", ");
    let succeeded = report
        .run
        .connectors_succeeded
        .iter()
        .map(|name| escape(name))
        .collect::<Vec<_>>()
        .join(", ");

    let script_hash = csp_script_hash(REPORT_SCRIPT);
    format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="color-scheme" content="dark light">
<meta name="referrer" content="no-referrer">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src '{script_hash}'; script-src-attr 'none'; style-src 'unsafe-inline'; img-src 'none'; font-src 'none'; connect-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'">
<title>{query} — BarnLabs Inquiry</title>
<style>{styles}</style>
</head>
<body>
<header class="masthead">
  <div class="shell">
    <div class="topbar">
      <p class="wordmark">Inquiry <small>by BarnLabs</small></p>
      <p class="run-id">Run {id}</p>
    </div>
    <div class="hero">
      <div>
        <p class="kicker">Source-grounded research dossier</p>
        <h1>{query}</h1>
        <p class="lede">{summary}</p>
        <p class="hero-meta">{source_count} sources · {finding_count} findings · generated {created}</p>
      </div>
      <aside class="assessment" aria-labelledby="evidence-label">
        <p class="assessment-status">{evidence_status}</p>
        <h2 id="evidence-label">{evidence_label}</h2>
        <p class="assessment-explanation">{evidence_explanation}</p>
        <dl class="evidence-dimensions">
          <div><dt>Source coverage</dt><dd class="{source_coverage_class}">{source_coverage}<small>Legacy coverage signal: {confidence}</small></dd></div>
          <div><dt>Publisher diversity</dt><dd class="{publisher_diversity_class}">{publisher_diversity}</dd></div>
          <div><dt>Freshness</dt><dd class="{freshness_class}">{freshness}</dd></div>
          <div><dt>Identity binding</dt><dd class="{identity_binding_class}">{identity_binding}</dd></div>
          <div><dt>Media rights</dt><dd class="{media_rights_class}">{media_rights}</dd></div>
        </dl>
      </aside>
    </div>
  </div>
</header>
<main class="shell">
  <section class="dossier-section controls-section" aria-labelledby="plan-heading">
    <div class="section-head">
      <p class="section-label">01 · Research plan</p>
      <div>
        <h2 id="plan-heading">Navigate the evidence</h2>
        <p>Search tables, findings, and the source ledger together, or narrow evidence by facet.</p>
      </div>
    </div>
    <p class="research-rationale">{rationale}</p>
    <div class="report-tools">
      <div class="search-control">
        <label for="reportSearch">Search this dossier</label>
        <input id="reportSearch" type="search" autocomplete="off" spellcheck="false" placeholder="Find in tables, findings, or sources" aria-describedby="filterStatus">
      </div>
      <div>
        <span class="facet-label" id="facetLabel">Filter findings and metrics by facet</span>
        <div class="filters" role="group" aria-labelledby="facetLabel">
          <button class="filter active" type="button" data-filter="all" aria-pressed="true">All facets</button>
          {facets}
        </div>
      </div>
      <p class="filter-status" id="filterStatus" role="status" aria-live="polite"></p>
    </div>
    <noscript><p class="quiet">Search and facet controls require JavaScript; all evidence remains visible without it.</p></noscript>
  </section>
  {metric_section}
  {table_content}
  <section class="dossier-section" aria-labelledby="findings-heading">
    <div class="section-head">
      <p class="section-label">02 · Evidence</p>
      <div>
        <h2 id="findings-heading">Findings</h2>
        <p>Each statement retains its support level, content trust, facet, tags, and source references.</p>
      </div>
    </div>
    <div class="findings">{findings}</div>
    <p class="empty-state" id="findingsEmpty" {findings_empty_hidden}>No findings match the current search and facet.</p>
  </section>
  <section class="dossier-section" aria-labelledby="limits-heading">
    <div class="section-head">
      <p class="section-label">03 · Limits</p>
      <div>
        <h2 id="limits-heading">Warnings and boundaries</h2>
        <p>Run-specific cautions are evidence, not decoration.</p>
      </div>
    </div>
    {warning_content}
  </section>
  <section class="dossier-section source-ledger" aria-labelledby="sources-heading">
    <div class="section-head">
      <p class="section-label">04 · Audit trail</p>
      <div>
        <h2 id="sources-heading">Source ledger</h2>
        <p>Publisher, record identifier, retrieval status, quality tier, attached files, and terms remain inspectable.</p>
      </div>
    </div>
    <div class="ledger-actions">
      <button class="copy" id="copySources" type="button">Copy all citations</button>
      <span class="copy-status" id="copyStatus" role="status" aria-live="polite"></span>
    </div>
    <div class="table-wrap" tabindex="0" aria-label="Source ledger">
      <table>
        <thead><tr><th scope="col">Source</th><th scope="col">Provenance time</th><th scope="col">Tier</th><th scope="col">License / terms</th></tr></thead>
        <tbody>{sources}</tbody>
      </table>
    </div>
    <p class="empty-state" id="sourcesEmpty" {sources_empty_hidden}>No sources match the current search.</p>
  </section>
  <section class="dossier-section" aria-labelledby="run-heading">
    <div class="section-head">
      <p class="section-label">05 · Run record</p>
      <div>
        <h2 id="run-heading">Reproducibility</h2>
        <p>What ran, whether it used the network, and which connectors returned evidence.</p>
      </div>
    </div>
    <dl class="provenance">
      <div><dt>Schema</dt><dd>{schema}</dd></div>
      <div><dt>Engine</dt><dd>{engine}</dd></div>
      <div><dt>Network</dt><dd>{network}</dd></div>
      <div><dt>Duration</dt><dd>{duration} ms</dd></div>
      <div><dt>Attempted</dt><dd>{attempted}</dd></div>
      <div><dt>Succeeded</dt><dd>{succeeded}</dd></div>
    </dl>
    {connector_error_section}
  </section>
</main>
<footer class="page-footer shell">
  <span>BarnLabs Inquiry · Evidence before certainty.</span>
  <span>Generated {created}</span>
</footer>
<script>{script}</script>
</body>
</html>"##,
        script_hash = script_hash,
        styles = REPORT_STYLES,
        script = REPORT_SCRIPT,
        query = escape(&report.query),
        id = escape(&report.id.to_string()),
        summary = escape(&report.summary),
        confidence = escape(&report.confidence.to_string()),
        evidence_status = escape(&report.evidence.status.to_string()),
        evidence_label = escape(&report.evidence.label),
        evidence_explanation = escape(&report.evidence.explanation),
        source_coverage = escape(&report.evidence.source_coverage.to_string()),
        source_coverage_class = attr(
            &report
                .evidence
                .source_coverage
                .to_string()
                .replace(' ', "-")
        ),
        publisher_diversity = escape(&report.evidence.publisher_diversity.to_string()),
        publisher_diversity_class = attr(
            &report
                .evidence
                .publisher_diversity
                .to_string()
                .replace(' ', "-")
        ),
        freshness = escape(&report.evidence.freshness.to_string()),
        freshness_class = attr(&report.evidence.freshness.to_string().replace(' ', "-")),
        identity_binding = escape(&report.evidence.identity_binding.to_string()),
        identity_binding_class = attr(
            &report
                .evidence
                .identity_binding
                .to_string()
                .replace(' ', "-")
        ),
        media_rights = escape(&report.evidence.media_rights.to_string()),
        media_rights_class = attr(&report.evidence.media_rights.to_string().replace(' ', "-")),
        source_count = report.sources.len(),
        finding_count = report.findings.len(),
        created = report.created_at.format("%Y-%m-%d %H:%M UTC"),
        facets = facets,
        rationale = escape(&report.plan.rationale),
        table_content = table_content,
        findings = findings,
        findings_empty_hidden = if report.findings.is_empty() {
            ""
        } else {
            "hidden"
        },
        warning_content = warning_content,
        sources = sources,
        sources_empty_hidden = if report.sources.is_empty() {
            ""
        } else {
            "hidden"
        },
        schema = escape(&report.schema_version),
        engine = escape(&report.run.engine_version),
        network = if report.run.network_used {
            "used"
        } else {
            "offline"
        },
        duration = (report.run.completed_at - report.run.started_at).num_milliseconds(),
        attempted = if attempted.is_empty() {
            "none"
        } else {
            &attempted
        },
        succeeded = if succeeded.is_empty() {
            "none"
        } else {
            &succeeded
        },
        connector_error_section = if connector_errors.is_empty() {
            String::new()
        } else {
            format!(
                r#"<div class="connector-errors"><h3>Connector errors</h3><ul class="warnings">{connector_errors}</ul></div>"#
            )
        },
        metric_section = if report.metrics.is_empty() {
            String::new()
        } else {
            format!(
                r#"<section class="dossier-section" aria-labelledby="metrics-heading"><div class="section-head"><p class="section-label">Measured observations</p><div><h2 id="metrics-heading">Metrics</h2><p>Periods, units, and source references stay attached. Bars compare only metrics with the same unit.</p></div></div><div class="metrics">{metrics}</div></section>"#
            )
        }
    )
}

fn table_column_label(column: &crate::model::TableColumn) -> String {
    column
        .unit
        .as_deref()
        .map(|unit| format!("{} ({unit})", column.label))
        .unwrap_or_else(|| column.label.clone())
}

fn csp_script_hash(script: &str) -> String {
    let digest = Sha256::digest(script.as_bytes());
    format!("sha256-{}", base64_standard(&digest))
}

fn base64_standard(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or_default();
        let third = chunk.get(2).copied().unwrap_or_default();
        encoded.push(ALPHABET[(first >> 2) as usize] as char);
        encoded.push(ALPHABET[(((first & 0b0000_0011) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(ALPHABET[(((second & 0b0000_1111) << 2) | (third >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(ALPHABET[(third & 0b0011_1111) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

pub fn validate_report(report: &ResearchReport) -> Result<()> {
    if report.schema_version != "inquiry.report/v1" {
        anyhow::bail!("unsupported report schema: {}", report.schema_version);
    }
    let mut source_ids = HashSet::new();
    for source in &report.sources {
        if !source_ids.insert(source.id.as_str()) {
            anyhow::bail!("duplicate source id: {}", source.id);
        }
        if safe_http_url(&source.url).is_none() {
            anyhow::bail!("source '{}' has a non-HTTP(S) or invalid URL", source.id);
        }
        for (field, url) in [
            ("content_url", source.provenance.content_url.as_deref()),
            ("preview_url", source.provenance.preview_url.as_deref()),
        ] {
            if let Some(url) = url
                && safe_http_url(url).is_none()
            {
                anyhow::bail!("source '{}' has an invalid {field}", source.id);
            }
        }
    }
    for (kind, title, ids) in report
        .findings
        .iter()
        .map(|finding| {
            (
                "finding",
                finding.title.as_str(),
                finding.source_ids.as_slice(),
            )
        })
        .chain(report.metrics.iter().map(|metric| {
            (
                "metric",
                metric.label.as_str(),
                metric.source_ids.as_slice(),
            )
        }))
    {
        if ids.is_empty() {
            anyhow::bail!("{kind} '{title}' has no source reference");
        }
        for id in ids {
            if !source_ids.contains(id.as_str()) {
                anyhow::bail!("{kind} '{title}' references missing source id '{id}'");
            }
        }
    }
    let mut table_ids = HashSet::new();
    for table in &report.tables {
        if !table_ids.insert(table.id.as_str()) {
            anyhow::bail!("duplicate table id: {}", table.id);
        }
        if table.columns.is_empty() {
            anyhow::bail!("table '{}' has no columns", table.title);
        }
        if table.columns.len() > 32 {
            anyhow::bail!("table '{}' exceeds the 32-column limit", table.title);
        }
        if table.rows.len() > 10_000 {
            anyhow::bail!("table '{}' exceeds the 10,000-row limit", table.title);
        }
        if table.source_ids.is_empty() {
            anyhow::bail!("table '{}' has no source reference", table.title);
        }
        for id in &table.source_ids {
            if !source_ids.contains(id.as_str()) {
                anyhow::bail!(
                    "table '{}' references missing source id '{id}'",
                    table.title
                );
            }
        }
        let mut row_ids = HashSet::new();
        for row in &table.rows {
            if !row_ids.insert(row.id.as_str()) {
                anyhow::bail!("table '{}' has duplicate row id '{}'", table.title, row.id);
            }
            if row.cells.len() != table.columns.len() {
                anyhow::bail!(
                    "table '{}' row '{}' has {} cells for {} columns",
                    table.title,
                    row.id,
                    row.cells.len(),
                    table.columns.len()
                );
            }
        }
    }
    Ok(())
}

fn safe_http_url(value: &str) -> Option<&str> {
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then_some(value)
}

fn provenance_time_label(source: &crate::model::SourceRecord) -> String {
    if matches!(source.quality, SourceQuality::DiscoveryOnly)
        && source.content_hash.is_none()
        && source.provenance.request_url.is_none()
    {
        return source
            .provenance
            .source_updated_at
            .as_deref()
            .and_then(|value| value.strip_prefix("curated registry reviewed "))
            .map(|date| format!("Registry reviewed {date}; not retrieved in this run"))
            .unwrap_or_else(|| "Not retrieved in this run".into());
    }
    format!(
        "Retrieved {}",
        source.retrieved_at.format("%Y-%m-%d %H:%M UTC")
    )
}

fn safe_preview_link_url(value: &str) -> bool {
    let Ok(parsed) = url::Url::parse(value) else {
        return false;
    };
    parsed.scheme() == "https"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.port().is_none()
        && matches!(
            parsed.host_str(),
            Some("upload.wikimedia.org" | "commons.wikimedia.org")
        )
}

fn source_link(source: &crate::model::SourceRecord, label: &str) -> String {
    match safe_http_url(&source.url) {
        Some(url) => format!(
            r#"<a href="{}" target="_blank" rel="noopener noreferrer">{}</a>"#,
            attr(url),
            escape(label)
        ),
        None => format!(
            r#"<span title="Invalid or non-HTTP(S) source link rejected">{}</span>"#,
            escape(label)
        ),
    }
}

fn external_link(url: &str, label: &str) -> String {
    match safe_http_url(url) {
        Some(url) => format!(
            r#"<a href="{}" target="_blank" rel="noopener noreferrer">{}</a>"#,
            attr(url),
            escape(label)
        ),
        None => format!("<span>{}</span>", escape(label)),
    }
}

pub fn default_report_path(
    query: &str,
    id: uuid::Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
    extension: &str,
) -> PathBuf {
    let mut slug = query
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug = slug.trim_matches('-').chars().take(52).collect();
    if slug.is_empty() {
        slug = "research".into();
    }
    let timestamp = created_at.format("%Y%m%dT%H%M%SZ");
    let short_id = id.simple().to_string().chars().take(8).collect::<String>();
    PathBuf::from("reports").join(format!("{slug}.{timestamp}.{short_id}.inquiry.{extension}"))
}

fn escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn attr(input: &str) -> String {
    escape(input)
}
fn title_case(input: &str) -> String {
    let mut chars = input.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{EngineConfig, ResearchEngine};
    use crate::model::{ResearchRequest, TableArtifact, TableColumn, TableRow};

    async fn report_for_table_tests() -> ResearchReport {
        let engine = ResearchEngine::new(EngineConfig {
            network: false,
            searxng_url: None,
        })
        .unwrap();
        let mut report = engine
            .research(ResearchRequest::new("dengue transmission safety"))
            .await
            .unwrap();
        report.tables.clear();
        report
    }

    fn sample_table(source_id: &str) -> TableArtifact {
        TableArtifact {
            id: "sample-table".into(),
            title: "Sample measurements".into(),
            description: "A compact validation fixture.".into(),
            columns: vec![TableColumn {
                key: "measurement".into(),
                label: "Measurement".into(),
                unit: Some("mm".into()),
            }],
            rows: vec![TableRow {
                id: "sample-row".into(),
                cells: vec!["12.5".into()],
            }],
            source_ids: vec![source_id.into()],
            notes: vec!["Fixture note".into()],
        }
    }

    #[test]
    fn report_paths_are_sanitized() {
        let id = uuid::Uuid::parse_str("12345678-1234-4234-8234-1234567890ab").unwrap();
        let created = chrono::DateTime::parse_from_rfc3339("2026-07-16T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(
            default_report_path("../../A useful Query!", id, created, "html"),
            PathBuf::from("reports/a-useful-query.20260716T000000Z.12345678.inquiry.html")
        );
    }

    #[test]
    fn csp_hash_uses_standard_padded_base64() {
        assert_eq!(base64_standard(b""), "");
        assert_eq!(base64_standard(b"f"), "Zg==");
        assert_eq!(base64_standard(b"fo"), "Zm8=");
        assert_eq!(base64_standard(b"foo"), "Zm9v");
    }

    #[tokio::test]
    async fn rendered_csp_hash_matches_the_exact_inline_script() {
        let report = report_for_table_tests().await;
        let html = render_html(&report);
        let inline_script = html
            .split_once("<script>")
            .and_then(|(_, remainder)| remainder.split_once("</script>"))
            .map(|(script, _)| script)
            .expect("rendered report should contain its static script");
        let expected_hash = csp_script_hash(inline_script);

        assert_eq!(inline_script, REPORT_SCRIPT);
        assert_eq!(html.matches("<script>").count(), 1);
        assert!(html.contains(&format!(
            "script-src '{expected_hash}'; script-src-attr 'none'"
        )));
        assert!(!html.contains("script-src 'unsafe-inline'"));
    }

    #[tokio::test]
    async fn rendered_tables_are_semantic_searchable_and_escaped() {
        let mut report = report_for_table_tests().await;
        let source_id = report.sources[0].id.clone();
        report.tables.push(TableArtifact {
            id: "unsafe-table".into(),
            title: "Voltage <script>alert('title')</script> & facts".into(),
            description: r#"<img src=x onerror="alert('description')">"#.into(),
            columns: vec![
                TableColumn {
                    key: "potential".into(),
                    label: "Potential <nominal>".into(),
                    unit: Some(r#"V" data-pwned="yes"#.into()),
                },
                TableColumn {
                    key: "note".into(),
                    label: "Operator's note".into(),
                    unit: None,
                },
            ],
            rows: vec![TableRow {
                id: r#"row" data-pwned="true"#.into(),
                cells: vec!["<script>alert('cell')</script>".into(), "A&B < C".into()],
            }],
            source_ids: vec![source_id],
            notes: vec!["<b>not markup</b>".into()],
        });

        validate_report(&report).unwrap();
        let html = render_html(&report);

        assert!(html.contains(r#"<caption class="visually-hidden">Voltage &lt;script&gt;alert(&#39;title&#39;)&lt;/script&gt; &amp; facts</caption>"#));
        assert!(html.contains(
            r#"<th scope="col">Potential &lt;nominal&gt; (V&quot; data-pwned=&quot;yes)</th>"#
        ));
        assert!(
            html.contains(
                r#"data-label="Potential &lt;nominal&gt; (V&quot; data-pwned=&quot;yes)""#
            )
        );
        assert!(html.contains(r#"data-row-id="row&quot; data-pwned=&quot;true""#));
        assert!(html.contains("&lt;script&gt;alert(&#39;cell&#39;)&lt;/script&gt;"));
        assert!(html.contains("A&amp;B &lt; C"));
        assert!(html.contains("&lt;b&gt;not markup&lt;/b&gt;"));
        assert!(html.contains(r#"id="tablesEmpty" hidden"#));
        assert!(
            REPORT_SCRIPT
                .contains("tableRows.forEach(row => { row.hidden = !matchesSearch(row, term); });")
        );
        assert!(REPORT_SCRIPT.contains("tablesEmpty.hidden = visibleTableRows !== 0"));
        assert!(!html.contains("<script>alert('title')</script>"));
        assert!(!html.contains("<script>alert('cell')</script>"));
        assert!(!html.contains("<img src=x"));

        report.tables[0].rows.clear();
        let empty_html = render_html(&report);
        assert!(empty_html.contains(r#"id="tablesEmpty" >No table rows"#));
    }

    #[tokio::test]
    async fn table_validation_enforces_identifiers_shape_sources_and_limits() {
        let mut report = report_for_table_tests().await;
        let source_id = report.sources[0].id.clone();
        let table = sample_table(&source_id);
        report.tables = vec![table.clone()];
        validate_report(&report).unwrap();

        let mut duplicate_table = report.clone();
        duplicate_table.tables.push(table.clone());
        assert!(
            validate_report(&duplicate_table)
                .unwrap_err()
                .to_string()
                .contains("duplicate table id")
        );

        let mut duplicate_row = report.clone();
        duplicate_row.tables[0].rows.push(table.rows[0].clone());
        assert!(
            validate_report(&duplicate_row)
                .unwrap_err()
                .to_string()
                .contains("duplicate row id")
        );

        let mut mismatched_row = report.clone();
        mismatched_row.tables[0].rows[0].cells.clear();
        assert!(
            validate_report(&mismatched_row)
                .unwrap_err()
                .to_string()
                .contains("has 0 cells for 1 columns")
        );

        let mut missing_source = report.clone();
        missing_source.tables[0].source_ids = vec!["missing-source".into()];
        assert!(
            validate_report(&missing_source)
                .unwrap_err()
                .to_string()
                .contains("references missing source id")
        );

        let mut no_source = report.clone();
        no_source.tables[0].source_ids.clear();
        assert!(
            validate_report(&no_source)
                .unwrap_err()
                .to_string()
                .contains("has no source reference")
        );

        let columns = (0..32)
            .map(|index| TableColumn {
                key: format!("column-{index}"),
                label: format!("Column {index}"),
                unit: None,
            })
            .collect::<Vec<_>>();
        let mut at_column_limit = report.clone();
        at_column_limit.tables[0].columns = columns.clone();
        at_column_limit.tables[0].rows[0].cells = vec!["value".into(); 32];
        validate_report(&at_column_limit).unwrap();

        let mut over_column_limit = at_column_limit;
        over_column_limit.tables[0].columns.push(TableColumn {
            key: "column-32".into(),
            label: "Column 32".into(),
            unit: None,
        });
        over_column_limit.tables[0].rows[0]
            .cells
            .push("value".into());
        assert!(
            validate_report(&over_column_limit)
                .unwrap_err()
                .to_string()
                .contains("exceeds the 32-column limit")
        );

        let rows = (0..10_000)
            .map(|index| TableRow {
                id: format!("row-{index}"),
                cells: vec![index.to_string()],
            })
            .collect::<Vec<_>>();
        let mut at_row_limit = report.clone();
        at_row_limit.tables[0].rows = rows;
        validate_report(&at_row_limit).unwrap();

        at_row_limit.tables[0].rows.push(TableRow {
            id: "row-10000".into(),
            cells: vec!["10000".into()],
        });
        assert!(
            validate_report(&at_row_limit)
                .unwrap_err()
                .to_string()
                .contains("exceeds the 10,000-row limit")
        );
    }

    #[tokio::test]
    async fn rendered_dossier_has_hashed_script_search_and_plain_wordmark() {
        let engine = ResearchEngine::new(EngineConfig {
            network: false,
            searxng_url: None,
        })
        .unwrap();
        let report = engine
            .research(ResearchRequest::new("dengue transmission safety"))
            .await
            .unwrap();
        let html = render_html(&report);
        let expected_directive = format!("script-src '{}'", csp_script_hash(REPORT_SCRIPT));

        assert!(html.contains(&expected_directive));
        assert!(html.contains("script-src-attr 'none'"));
        assert!(html.contains("img-src 'none'"));
        assert!(html.contains(&format!("<script>{REPORT_SCRIPT}</script>")));
        assert!(html.contains(r#"id="reportSearch""#));
        assert!(html.contains(r#"data-search-item="finding""#));
        assert!(html.contains(r#"data-search-item="source""#));
        assert!(html.contains("@media print"));
        assert!(html.contains(r#"class="wordmark""#));
        assert!(html.contains(&escape(&report.evidence.label)));
        assert!(html.contains("Publisher diversity"));
        assert!(html.contains("Identity binding"));
        assert!(html.contains("Media rights"));
        assert!(html.contains("Legacy coverage signal"));
        assert!(!html.contains("answer correctness"));
        assert!(!html.contains("<svg"));
        assert!(!html.contains("Evidence cards"));
    }

    #[tokio::test]
    async fn rejects_unsafe_links_and_broken_source_references() {
        let engine = ResearchEngine::new(EngineConfig {
            network: false,
            searxng_url: None,
        })
        .unwrap();
        let mut report = engine
            .research(ResearchRequest::new("dengue transmission safety"))
            .await
            .unwrap();
        report.sources[0].url = "javascript:alert(1)".into();
        assert!(validate_report(&report).is_err());
        assert!(!render_html(&report).contains("href=\"javascript:"));

        report.sources[0].url = "https://example.test/source".into();
        report.findings[0].source_ids = vec!["missing-source".into()];
        assert!(validate_report(&report).is_err());
    }

    #[test]
    fn remote_preview_links_are_restricted_to_known_https_hosts() {
        assert!(safe_preview_link_url(
            "https://upload.wikimedia.org/example.jpg"
        ));
        assert!(safe_preview_link_url(
            "https://commons.wikimedia.org/wiki/Special:Redirect/file/example.jpg"
        ));
        assert!(!safe_preview_link_url("http://upload.wikimedia.org/x.jpg"));
        assert!(!safe_preview_link_url(
            "https://user@upload.wikimedia.org/x.jpg"
        ));
        assert!(!safe_preview_link_url(
            "https://upload.wikimedia.org:444/x.jpg"
        ));
        assert!(!safe_preview_link_url("https://tracker.example/x.gif"));
    }

    #[tokio::test]
    async fn rendered_reports_do_not_auto_load_remote_media() {
        let engine = ResearchEngine::new(EngineConfig {
            network: false,
            searxng_url: None,
        })
        .unwrap();
        let mut report = engine
            .research(ResearchRequest::new("human heart 3d model"))
            .await
            .unwrap();
        report.sources[0].provenance.preview_url =
            Some("https://upload.wikimedia.org/example.jpg".into());
        report.findings[0].source_ids = vec![report.sources[0].id.clone()];
        let html = render_html(&report);
        assert!(!html.contains("<img"));
        assert!(html.contains("Inquiry does not load remote media"));
    }

    #[tokio::test]
    async fn curated_discovery_links_are_not_labeled_as_retrieved() {
        let engine = ResearchEngine::new(EngineConfig {
            network: false,
            searxng_url: None,
        })
        .unwrap();
        let report = engine
            .research(ResearchRequest::new(
                "OpenStax integration by parts textbook section",
            ))
            .await
            .unwrap();
        let html = render_html(&report);
        assert!(html.contains("Registry reviewed 2026-07-16; not retrieved in this run"));
        assert!(!html.contains(">2026-07-16 00:00 UTC<"));
    }
}
