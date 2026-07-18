# Public-source registry

Retrieval and review date: **2026-07-17**. Source terms can change; connector owners must re-verify them before release and at least annually.

| Source | Domain | v0.1 state | Access and main constraint |
|---|---|---|---|
| World Bank Indicators | Finance, locations, health, statistics | Live connector | No key; dataset-specific license, periods can differ |
| MedlinePlus Web Service | Disease, symptoms, transmission, consumer health | Live connector | No key; attribute MedlinePlus.gov, do not imply endorsement; updated Tuesday-Saturday |
| openFDA drug labels | Medication-label evidence | Live explicit tool | No key used; 240 requests/minute and 1,000/day per IP; submitted labeling is not verified by FDA and may not match a distributed product |
| Wikimedia Commons Action API | Anatomy images, diagrams, media, open assets | Live connector | File-specific licenses/attribution; extmetadata can be incomplete or HTML-formatted |
| NASA 3D Resources | Space/science models and print assets | Live connector | Official catalog match; verify asset format, scale, printability, and NASA/resource-specific usage rules |
| NIH 3D | Biomedical models and print assets | Query-specific catalog link; direct adapter planned | Each model's validation, intended use, category, files, and license must be inspected |
| SEC EDGAR Data APIs | Public-company financials | Planned | No key; identify user agent, maximum 10 requests/second |
| OpenFEMA | Disasters and safety | Catalog link; planned adapter | No key; records are historical/administrative, not forecasts |
| UK Police Data | Jurisdiction-specific safety | Planned reference adapter | OGL v3; 15 requests/second, approximate locations and coverage gaps |
| OSM / public Nominatim | Locations | Live explicit resolver | ODbL; maximum 1 request/second, no autocomplete/bulk use; candidates are not asserted matches |
| WHO GHO / Disease Outbreak News | Disease and transmission | Catalog link; planned adapter | Dataset/publication terms vary; outbreak news is not exhaustive |
| Wikidata | Current and numbered officeholder resolution | Live connector | CC0 structured data; community claims vary and require official or archival corroboration |
| UK Parliament regnal records | Current UK monarch corroboration | Live only for exact current-UK-monarch routing | Official current-reign CSV records; exact Wikidata identity and dates must agree before acceptance |
| The Royal Family | Current UK monarch profile | Live only for exact current-UK-monarch routing | Institution-controlled primary profile, not independent biographical appraisal |
| NASA EONET v3 open events | Natural-event snapshots | Permission-gated live explicit tool | No key; fixed `status=open&limit=50` request to the exact HTTPS endpoint, 1 MiB response cap, geometry/source timestamps preserved, 429 `Retry-After` surfaced, no automatic retry or polling; curated and not comprehensive or official for event extent |
| FAA NAS airport events | U.S. airport operations | Live explicit tool | No key; one exact HTTPS endpoint, 1 MiB response cap, source timestamps, 429 `Retry-After` surfaced, no automatic retry; airport-level only |
| FAA Releasable Aircraft Database | U.S. registration | User-supplied local archive | Local single N-number lookup; archive hash and local timestamp recorded; owner/address, serial, Mode S, coordinates, history, bulk, and reverse-owner data omitted |
| Official airline status pages | Individual flight status | Handoff only | No retrieval; carrier must be selected explicitly; no position, history, polling, alert, or status claim |
| USPS, UPS, FedEx, and DHL tracking pages | Package tracking | Handoff only | No retrieval; identifier omitted from URL by default; no carrier inference, scraping, control bypass, or delivery-state claim |
| OpenStax | Open textbooks | Catalog link plus a curated integration-by-parts discovery link | Not retrieved in the current run; verify the page, edition, license, and current use restrictions before relying on or reusing it |
| NIST constants / SP 811 | Formulas, measurements, conversions | Planned versioned local table | Preserve version, units, uncertainty, and source |
| Wikipedia | General orientation | Live connector | CC BY-SA; secondary orientation, follow references |
| OpenAlex | Scholarly discovery | Live connector | Metadata CC0; linked work license varies |
| Open Library | Book discovery | Live connector | Availability is not equivalent to open licensing |
| SearXNG | Discovery | Optional live connector | User-supplied HTTPS endpoint, with HTTP allowed only on loopback for development; the approved query is sent to that service; cite original publishers, not the metasearch layer |

## Expanded adapter research backlog

Each item needs official API/terms verification before code:

- **News:** GDELT and publisher RSS/Atom, clustered by event and separated from source claims; archives and current-news cutoffs visible.
- **Law:** GovInfo, Congress.gov, CourtListener/RECAP, state legislative sites, EUR-Lex, and official gazettes. Every result needs jurisdiction, authority, status, effective/version date, court and docket/citation where applicable. Inquiry must never describe old or proposed text as current law.
- **Standards and engineering:** NIST, NASA handbooks, public government standards, manufacturer datasheets, and standards-body catalogs. Many full standards are copyrighted; return identifiers, scope, edition, official links, and only lawfully reusable excerpts/data.
- **Parts and measurements:** manufacturer and distributor data with exact manufacturer part number, thread system, nominal/actual dimensions, tolerance, material/grade, voltage/current/frequency, safety certification, and datasheet revision. Never merge near-matches.
- **Chemistry:** PubChem and NIST Chemistry WebBook, preserving compound identifiers, formula, charge/isotope, conditions, units, uncertainty, and source version.
- **Psychology and medicine:** PubMed/NCBI, Crossref/OpenAlex, WHO, CDC, ClinicalTrials.gov, and open-access repositories. Distinguish metadata, abstract, full text, preprint, review, and guideline; preserve study design and sample.
- **Anatomy and images:** official/open educational collections with exact species/body region/view and reuse license. Do not copy restricted clinical imagery or identifying patient media.
- **3D/CAD/print files:** NIH 3D, NASA 3D, Smithsonian 3D, Printables/Thingiverse/GrabCAD only where API/terms allow. Record asset license, author, canonical page, dimensions/units, file format, print/manufacturing assumptions, safety caveats, and content hash.
- **Country standards and cards:** ISO/IEC catalogs, national standards bodies, ICAO and government issuers, plus manufacturer specs. Full copyrighted standards are linked, not reproduced.
- **GitHub:** official GitHub search/API or local `gh`, with repository license, release/tag/commit, language, maintenance signals, archived status, and dependency/security warnings. Popularity is not quality.

## Connector acceptance checklist

- current official API documentation and terms recorded;
- user agent, request ceiling, timeout, retry/backoff, and cache TTL defined;
- dataset/license captured per result rather than inferred from publisher;
- exact entity/part/place resolution happens before aggregation;
- geography, period, unit, denominator, missingness, revision, and uncertainty preserved;
- representative success, empty, malformed, rate-limit, timeout, and wrong-entity tests;
- no person-level or restricted data by default;
- derived metrics reproduce from stored inputs and a named transform;
- original publisher is the citation target when a search/crawler discovers it.

Firecrawl and SearXNG are retrieval layers, not evidence sources. Inquiry favors narrow official adapters because a broad scraper creates more licensing, provenance, prompt-injection, and reliability risk than decision value in v0.1.
