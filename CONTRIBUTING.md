# Contributing

Inquiry welcomes focused connectors, calculations, report improvements, platform clients, export adapters, tests, and documentation.

Before a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
swift test
./script/test_mcp.sh
```

New connectors must complete the checklist in `docs/data-sources.md`, include official documentation and license/terms links, implement bounded timeouts and rate behavior, and add representative tests without real personal data. New derived metrics must preserve inputs, units, periods, formula/transform identifiers, and known-value tests.

Do not contribute scraping of authenticated services, person-level scoring, de-anonymization, diagnostic logic, autonomous transactions, copied standards/textbooks/assets without an exact compatible license, or model-generated citations.

