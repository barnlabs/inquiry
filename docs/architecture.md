# Architecture and trust model

Inquiry is a local process, not a hosted data lake. Its core artifact is a structured research report with a provenance ledger and run record.

## Components

1. **Policy review** rejects narrow classes of sensitive targeting and adds health, finance, copyright, and people-research warnings.
2. **Planner** deterministically routes a natural-language query into facets. An optional local model may later propose the same typed plan; its output must validate and is never evidence.
3. **Entity resolver** separates candidates before retrieval. A place candidate includes name, full address/administrative hierarchy, coordinates, source identifier, and match rationale. A part/standard candidate includes manufacturer/issuer, exact identifier, edition, jurisdiction, units, and tolerance context.
4. **Connectors** call bounded public endpoints with an identifying user agent and connector-specific timeouts. Each connector returns normalized findings, metrics, assets, and sources.
5. **Normalizer** deduplicates source records and retains units, periods, publisher, retrieval time, license note, quality tier, and content hash where possible.
6. **Report renderer** creates JSON or self-contained HTML. HTML uses no analytics, cookies, remote fonts, remote image loads, or JavaScript CDN. External media is represented by click-only links.
7. **InquiryStudy indexer** reads one explicitly selected local directory through bounded document parsers, stores root-relative source segments and hashes, and performs deterministic cited search without connector calls.
8. **Interactive artifact renderer** validates supplied timeline events and credential-free HTTPS citations, then writes self-contained HTML with local filtering, copying, and CSV export.
9. **Interfaces** are a Rust CLI, MCP stdio server, native macOS SwiftUI application, and export adapters.

## Trust boundaries

- Natural-language input is untrusted and length-bounded by the caller.
- Package, flight, live-aircraft, and aircraft-registration identifiers are routed locally to scoped abstention before the general connector catalog or connector support checks run.
- Connector responses are untrusted external data. They are parsed as data and HTML-escaped before report rendering.
- Discovery indexes are not primary evidence and are labeled accordingly.
- URLs are accepted only for HTTP(S) discovery results.
- Downloadable assets require an explicit license and canonical source record; no silent mirroring.
- The MCP server writes a report only when the agent explicitly calls `render_report` with a path.
- MCP agents may search a prebuilt InquiryStudy index only after the operator explicitly enables the private-data tools. The host/model may receive returned excerpts. The server confines paths to a real `reports/` directory and cannot select or index arbitrary filesystem paths.
- Local course files are hostile parser input. Symlinks and special files are skipped, supported types are signature-checked, Office packages are never expanded to disk, macro payloads and XML declarations are rejected, and byte/page/archive/segment ceilings fail closed.
- InquiryStudy citations consist of a root-relative path, precise locator, normalized extracted excerpt checksum, and original-document checksum. Loaded indexes are revalidated before search; these checks detect inconsistency but do not authenticate a separately editable index against the original files. No matching span means no result or card.
- Timeline fields are untrusted supplied data. Text is HTML-escaped, sources require credential-free HTTPS, remote images are not loaded, and the generated page uses a restrictive Content Security Policy.
- The macOS client launches only the bundled Inquiry binary; it does not execute arbitrary commands.
- No local model, API key, browser login, or remote service is required for offline use.
- FAA airport status is a separate exact-host connector. Airline and package status use no-network official handoffs. FAA registration lookup operates on one local user-supplied archive and deliberately minimizes the returned fields.

## Provenance contract

The v1 report schema records source ID, canonical URL, publisher, retrieval time, publication/data period when known, license note, source type, quality tier, content hash when available, dataset ID, exact request URL, methodology URL, observation period, source update date, and optional asset content/preview URLs, format, and byte size. Still-planned additions include explicit jurisdiction/entity type, standard edition, required-attribution fields, denominator, uncertainty, measurements/tolerances, and transform identifiers.

## Confidence

Report-level confidence currently measures source coverage, not truth. Individual findings keep their own confidence. A high report confidence does not make an individual claim high-confidence and must never be presented as such.

## Local-model extension

A future planner adapter may accept only a JSON plan: query, facets, entities, geography, dates, and proposed connectors. The deterministic policy layer would validate the plan, connectors would fetch evidence, and the engine would ignore any model-written citations or numeric answers. No transformer runtime or weights ship in v0.1. A future optional profile may target local 2B–4B-class models after runtime, quantization, model-license, privacy, and device benchmarks are published.

Medication-label evidence and study-pack generation remain separate from the broad research planner. The former accepts only one or two explicit medication names and returns selected openFDA label sections with source limitations; the latter transforms an already validated report and refuses discovery-only evidence.

InquiryStudy is also separate from the broad planner. It represents course material as “the material states,” not as verified external truth. Course metadata is optional, speaker notes require opt-in, script/style HTML is excluded, and queries and snippets are not added to research history. Per-segment risk labels block suspected assessments, secrets, private/restricted material, and embedded instructions from recall exports. Local data never inherits permission to flow into live connectors, but an explicitly enabled MCP host/model is a separate disclosure boundary. The current in-process parsers are bounded but not sandboxed out of process; PDF parser isolation, time/memory ceilings, and fuzzing remain hardening work before a broad public release.
