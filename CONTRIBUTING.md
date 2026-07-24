# Contributing

Inquiry welcomes focused connectors, calculations, report improvements, platform clients, export adapters, tests, and documentation. Start with the [README](README.md), then read the task-relevant material in `docs/`, especially [data-source requirements](docs/data-sources.md), [architecture](docs/architecture.md), and [privacy](docs/privacy.md).

## Choose a bounded change

Describe the user job, the smallest behavior that changes, its source or privacy constraints, and a public or synthetic way to verify it. New connectors need official documentation plus license/terms links, bounded timeouts and rate behavior, and representative tests with no real personal data. New derived metrics must preserve inputs, units, periods, formula or transform identifiers, and known-value tests.

Do not contribute authenticated scraping, person-level scoring, de-anonymization, diagnostic logic, autonomous transactions, copied standards, textbooks, or assets without an exact compatible license, or model-generated citations.

## Verify the path you changed

Run the focused checks first, then the applicable adjacent checks. The pull-request CI runs the Rust, MCP smoke, supply-chain, and Swift sets below; the live relevance benchmark runs only on the scheduled or manually dispatched workflow because it contacts public sources.

```bash
# Rust code or tests
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check
cargo audit --deny warnings

# macOS client
swift test

# MCP protocol or tool behavior
./script/test_mcp.sh

# Connector, provenance, or relevance work
./script/check_connector_flows.sh
./script/check_relevance_benchmark.sh

# Brand or MCP bundle changes
./script/check_brand.sh
./script/build_mcpb.sh
```

`cargo deny check` and `cargo audit --deny warnings` require `cargo-deny` and `cargo-audit`; CI installs pinned versions of both. `./script/build_mcpb.sh` requires `jq` and the generated `THIRD_PARTY_LICENSES.html`. The script names its output `dist/barnlabs-inquiry-darwin-arm64.mcpb`, but does not enforce the host or local release binary architecture. Run it only on a Darwin arm64 host with an arm64 local release binary; otherwise, do not treat its output as a valid arm64 package. Regenerate licenses only when the dependency or license input changes. Do not run live connector or release packaging checks just for a documentation-only change.

## Keep generated output local

`target/`, `.build/`, `dist/`, `reports/`, `audits/`, and `.tools/` are generated local output and are intentionally ignored. Do not delete another contributor's output or force-add it to a pull request. Commit a small reproducible fixture only when a test needs it, and place it with the owning test or documentation.

## Open a reviewable pull request

Work from a current `main` branch in a dedicated branch such as `contrib/short-description`. Keep each pull request focused, include the relevant verification output, update the affected documentation, and complete the repository pull-request template. Use `git diff --check` before requesting review. Do not use `git add .`, force-push, merge, tag, publish a release, or change repository settings as part of a contribution.

For a bug report, use the GitHub template with an exact version or commit and public or synthetic reproduction steps. Report vulnerabilities through the private security-advisory flow described in [SECURITY.md](SECURITY.md), not a public issue. Never include private notes, PHI, credentials, confidential investigations, restricted course material, or copied content that lacks a compatible license.
