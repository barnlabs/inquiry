# Product, provider, and identity research review

Research cutoff: **2026-07-17 America/New_York**. Retrieval date for every link below: **2026-07-17**.

## Research contract

- **Decision:** choose a defensible minor UI/identity direction and identify lawful free connector paths for flight, aircraft, and package lookups.
- **Audience:** Donovan, release reviewers, and future maintainers.
- **Evidence hierarchy:** official product documentation, government data/API documentation, provider terms, and official design/trademark guidance first; product/company sites only for claims about their own behavior; agent inference labeled explicitly.
- **Excluded:** paid or credentialed integration work, authenticated scraping, bot-control bypass, live tail tracking, production deployment, and legal conclusions.
- **Stop when:** a release path can state exactly what is supported, what data leaves the device, what evidence is returned, and when the product must abstain.

## Comparable-product patterns

| Product/category | Primary-source observation | Inquiry decision |
| --- | --- | --- |
| Perplexity | Its help center describes answer citations and user-selectable source scope: [how Perplexity works](https://www.perplexity.ai/help-center/en/articles/10352895-how-does-perplexity-work), [internal knowledge/source selection](https://www.perplexity.ai/help-center/en/articles/10352914-what-is-internal-knowledge-search) | Preserve visible sources and scope without treating citation presence as proof of correctness |
| Flightradar24 | Its technical overview distinguishes ADS-B, MLAT, satellite, radar, and estimated positions: [how tracking works](https://www.flightradar24.com/blog/inside-flightradar24/how-does-fr24-track-aircraft/). Its privacy policy describes blocking and personal-data implications: [flight-tracking privacy](https://free.flightradar24.com/privacy-policy-flight-tracking) | Do not imitate live movement maps; disclose source/method limits and block person-aircraft pattern-of-life work |
| Maltego | Its SDK reference centers graph entities and transforms: [SDK/API reference](https://docs.maltego.com/en/support/solutions/articles/15000062354-sdk-api-reference) | Keep graph and connector provenance available, but do not make graph density the default research hierarchy |
| AfterShip | Its help material describes event-triggered tracking notifications: [notification delivery](https://tracking-helpcenter.aftership.com/en/article/notification-delivery-3isj87/) | Inquiry does not poll or alert in this release; it returns an explicit carrier handoff only |
| Obsidian | Obsidian describes local files and open formats: [about](https://obsidian.md/about), [data storage](https://obsidian.md/help/data-storage) | Preserve local-first artifacts and user-chosen storage rather than creating a hosted account requirement |
| Zotero | Zotero emphasizes source capture, organization, and citations: [quick start](https://www.zotero.org/support/quick_start_guide), [why Zotero](https://www.zotero.org/why) | Keep provenance attached through report, export, and study transformations |
| Academic discovery | [Semantic Scholar](https://www.semanticscholar.org/product), [Google Scholar](https://scholar.google.com/intl/engb/scholar/about.html), and [Elicit](https://pro.elicit.com/solutions/systematic-reviews) present different search, citation, and systematic-review scopes | Label discovery metadata as discovery, preserve paper identity, and never imply a search result is an appraised conclusion |

The selected UI implication is a quieter primary workflow: query and result hierarchy first; source ledger, warnings, and remote-media privacy remain available but are not all promoted into the header. Loading, empty, offline, failure, and media-off states must remain explicit.

## Aviation provider decision

### Accepted paths

- The [FAA National Airspace System status site](https://nasstatus.faa.gov/) exposes a public structured [airport-events endpoint](https://nasstatus.faa.gov/api/airport-events). Inquiry retrieves one full snapshot from the exact HTTPS host, selects one three-letter airport locally, caps the body at 1 MiB, rejects redirects, preserves retrieval and source timestamps, and surfaces 429 `Retry-After` or 503 without automatic retry.
- The [FAA Releasable Aircraft Database](https://www.faa.gov/licenses_certificates/aircraft_certification/aircraft_registry/releasable_aircraft_download) is offered as a downloadable archive and described as refreshed daily. The [FAA data-field documentation](https://registry.faa.gov/database/ardata.pdf) defines its records. Automated retrieval was not made part of Inquiry; the user supplies a local archive, and Inquiry returns one minimized registration record with a file hash.
- Individual flight status remains an official airline-page handoff. Inquiry normalizes only one explicitly selected carrier and flight identifier and returns no state.

### Rejected path

OpenSky's [terms of use](https://opensky-network.org/about/terms-of-use) require written licensing for commercial or operational use. Its [REST documentation](https://openskynetwork.github.io/opensky-api/rest.html) also defines authenticated quotas and 429 retry behavior. Inquiry does not ship OpenSky as an operational connector, does not silently fall back to it, and does not market live aircraft coverage.

### Remaining limitations

- FAA airport events do not prove that an airport is operating normally and do not represent a specific airline flight.
- A local FAA archive timestamp is the timestamp of the user's copy, not a cryptographically authenticated FAA publication time.
- Official airline pages can change layout or impose bot controls; Inquiry does not automate them.
- No path supports live location, tail history, alerts, owner/passenger association, or bulk lookup.

## Package-provider decision

Official developer paths are not free unauthenticated status feeds:

- [USPS OAuth](https://developers.usps.com/Oauth) requires an access token.
- [UPS developer support](https://developer.ups.com/support) documents OAuth and the retirement of legacy access keys.
- [FedEx authorization](https://developer.fedex.com/api/en-dj/catalog/authorization/docs.html) requires client credentials.
- [DHL Shipment Tracking](https://developer.dhl.com/tracking?lang=zh-hant&language_content_entity=en) requires an API key and publishes request limits.

Inquiry therefore returns only official carrier destinations: [USPS](https://tools.usps.com/go/TrackConfirmAction), [UPS](https://www.ups.com/track?loc=en_US), [FedEx](https://www.fedex.com/en-us/tracking.html), and [DHL](https://www.dhl.com/global-en/home/tracking.html). The default URL contains no identifier. An explicit deep-link option may place the identifier on the exact official host, with a warning about browser history and carrier disclosure. Inquiry does not infer the carrier, scrape a page, bypass authentication or bot controls, or state a delivery status.

## Identity and confusion review

Three vector directions were produced and inspected at 16, 32, 64, 256, and 1024 px on light, dark, and monochrome surfaces:

1. **Focus Frame** — strongest match to the actual evidence/result layout; selected.
2. **Source Spine** — coherent but too close to common graph, molecular, and OSINT marks.
3. **Open Loop** — legible but too close to a search lens or letter Q.

The review artifact is [`brand/explorations/2026-07-17/review.html`](../brand/explorations/2026-07-17/review.html). The final Focus Frame adds a visible boundary on dark backgrounds and a separate 16–32 px optical source. Mechanical checks cover accessible SVG metadata, 16–1024 px dimensions, essential color separation, opaque app-icon output, and contrast. Visual inspection covered the exact raster wordmark, social art, app icon, both backgrounds, and both monochrome variants.

Apple's [app-icon guidance](https://developer.apple.com/design/human-interface-guidelines/app-icons) informed the dedicated opaque source and system-mask boundary. The USPTO explains [strong marks](https://www.uspto.gov/trademarks/basics/strong-trademarks), [likelihood of confusion](https://www.uspto.gov/trademarks/search/likelihood-confusion), and [federal searching](https://www.uspto.gov/trademarks/search/federal-trademark-searching). A preliminary search also found unrelated products using “Inquiry,” including an [education research app](https://punctuate.co.nz/work/inquiry), a [market-research firm](https://www.inquiry.com.pl/), and adjacent [InQery AI](https://inqery.com/). This supports the public **BarnLabs Inquiry** pairing and a cautious clearance posture; it is not legal advice or proof that a mark is registrable.

## Release decision

- Ship the scoped FAA airport connector, official airline/package handoffs, local minimized FAA archive lookup, coverage matrix, and identifier routing guard.
- Keep OpenSky, authenticated carrier APIs, live aircraft tracking, polling, alerts, and direct ChatGPT local MCP out of the release.
- Use Focus Frame as the selected identity and keep the three rejected directions as review evidence rather than public alternatives.
- Re-review provider terms and exact official destinations before release; provider errors remain unresolved state rather than inferred normal/ delivered/on-time status.
