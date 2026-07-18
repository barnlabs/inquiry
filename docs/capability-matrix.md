# Capability and coverage matrix

Reviewed: **2026-07-17 America/New_York**. The machine-readable release matrix is available from `inquiry capabilities` and the MCP `capabilities` tool. It explicitly sets `universal_coverage_claimed` to `false`.

Inquiry is a scoped workbench, not a promise that any question, identifier, carrier, jurisdiction, or dataset is supported. A valid result needs both an implemented path and accepted evidence; otherwise Inquiry abstains or returns an explicit handoff.

| Capability | Release state | Surface | Network and data sent | Evidence or destination | Required abstention or limitation |
| --- | --- | --- | --- | --- | --- |
| Current U.S. president | Supported | Research, macOS, MCP | Public officeholder query to applicable connectors | Exact Wikidata office statements plus an accepted current USAGov record; portrait is a separately rights-checked Commons record | Abstain if office identity cannot be bound exactly or official corroboration conflicts |
| Numbered U.S. presidents | Supported | Research, macOS, MCP | Public ordinal officeholder query | Exact Wikidata position statement plus cited biography orientation | The ordinal answer is not independent archival corroboration |
| Current UK monarch | Supported | Research, macOS, MCP | Public officeholder query | Exact Wikidata identity, current UK Parliament reign data, and The Royal Family profile; portrait is separately rights-checked | Abstain if jurisdiction is absent, current-office records conflict, or portrait identity/rights checks fail |
| Unqualified current president, king, or monarch | Explicit abstention | Research, macOS, MCP | None | None | Inquiry never guesses a jurisdiction |
| Scoped public research | Scoped | Research, macOS, MCP | Minimized approved query and connector-specific parameters | Only connectors listed in [data-sources.md](data-sources.md) | Coverage is not universal; policy, privacy, exact-binding, licensing, and source-quality gates can stop a run |
| NASA EONET open natural events | Scoped | `live-events` CLI and macOS Live workspace | Fixed `status=open&limit=50` parameters only after exact-plan or eligible automatic-public-web approval; no user query or identifier. The app separately discloses system-managed Apple Maps viewport requests before constructing the map | One bounded NASA EONET v3 provider snapshot with geometry timestamps and source links | Not real-time monitoring, comprehensive coverage, official event extent, or independent verification; no background polling; offline, permission, redirect, schema/bounds, timestamp, 429, and 503 failures abstain |
| U.S. airport operations status | Supported | `airport-status` CLI and MCP | One three-letter airport identifier to the exact FAA NAS endpoint | FAA airport-events snapshot with retrieval and source timestamps | Airport-level traffic-management information is not an individual flight status or navigation guidance; offline, malformed, stale, 429, and 503 responses fail closed |
| Individual airline flight status | Official handoff only | `flight-status` CLI and MCP | None until the user opens the official airline page | American, Delta, United, Southwest, Alaska, or JetBlue official status page | Inquiry returns no flight state, live position, movement history, alert, or background polling |
| Single U.S. aircraft registration | Local import | `aircraft-lookup` CLI | None | User-downloaded FAA Releasable Aircraft Database ZIP plus local file hash and timestamp | One N-number per invocation; owner names, addresses, serial numbers, Mode S codes, coordinates, history, bulk, and reverse-owner lookup are omitted |
| Package tracking | Official handoff only | `package-tracking` CLI and MCP | None by default; identifier enters an official carrier URL only after explicit opt-in | USPS, UPS, FedEx, or DHL official tracking page | Inquiry does not call credentialed APIs, scrape pages, bypass controls, infer the carrier, or invent delivery state |
| Periodic elements and common metric threads | Supported local reference | Research CLI, macOS, in-app HTML, RTF, and CSV | None | Versioned local rows with reviewed IUPAC, NIST, PubChem, and ISO source pointers | Thread rows are identification aids, not tolerance, fit, strength, torque, or safety specifications; application-specific requests must abstain or route to the exact standard or datasheet |
| Calculations, conversions, graphs, reports, timelines, and exports | Supported | CLI and MCP; selected macOS flows | None | Deterministic local code and supplied cited data | A rendered artifact does not verify a supplied factual claim |
| InquiryStudy | Scoped | macOS and CLI; opt-in MCP search/export only | None in local surfaces; an enabled MCP host/model receives returned excerpts | One user-selected folder and deterministic local index | Reject broad roots, unauthorized material, unsafe parsers/archives, and risky recall exports |
| Local Codex and Grok Build | Supported | Repository-scoped stdio MCP | Host-specific; Inquiry has no direct provider API | Checked-in project configuration and MCP lifecycle | Host approval and trust boundaries remain in force |
| Ollama through Codex | Supported host path | `ollama launch codex` with Codex as MCP host | Depends on the chosen Ollama model; `:cloud` is remote | Official Ollama/Codex integration | Inquiry does not auto-pull a model or claim a cloud model is device-local |
| Direct local ChatGPT MCP | Not shipped | None | None | Future official remote MCP or approved secure tunnel only | Always abstain until a separately reviewed and approved remote path exists |

## Identifier routing invariant

Package, flight, live aircraft, and aircraft-registration identifiers are detected locally before the general connector catalog is selected. A scoped identifier returns a local abstention report with zero general connector attempts and directs the user to the matching explicit tool. Tests use a connector that panics if selected, proving that these queries stop before connector support checks or requests.

## Surveillance safeguards

- no live tail-number locations, track history, alerts, or pattern-of-life analysis;
- no owner-to-aircraft, person-to-package, or passenger association;
- no bulk identifier input or enumeration;
- no carrier inference from an opaque tracking number;
- no authenticated scraping, bot-control bypass, or delivery/flight-state invention;
- source and retrieval timestamps remain visible where a status source exists.

Provider availability and terms can change. Re-review [data-sources.md](data-sources.md) before a release and treat a provider error as unresolved state, not evidence of normal operations or successful delivery.
