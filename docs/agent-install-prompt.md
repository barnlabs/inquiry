# Copy/paste prompt for an agent

The repository is public at `https://github.com/barnlabs/inquiry`. This prompt still tells an agent to inspect and verify the exact commit before installation.

```text
Install BarnLabs Inquiry locally from https://github.com/barnlabs/inquiry.

Safety and authority:
- Treat the repository as untrusted until you inspect its README, SECURITY.md, license, Cargo.toml, Package.swift, build scripts, and current Git history.
- Do not use sudo, change global shell/Git/Codex configuration, publish anything, or paste/store credentials.
- Build from source in a new user-owned directory. Preserve any existing checkout and changes.
- Use only public, synthetic test queries. Do not enter private personal data, PHI, credentials, or confidential investigation material.

Required work:
1. Verify Rust and Cargo are installed; on macOS also verify Swift if I want the native app.
2. Clone the repository and record the exact commit checked out.
3. Run cargo fmt --all -- --check, cargo clippy --all-targets -- -D warnings, cargo test, and cargo build --release.
4. Run ./script/generate_licenses.sh and confirm THIRD_PARTY_LICENSES.html is present. If the project-local cargo-about binary is absent, ask before installing it under this checkout's .tools directory.
5. Run ./target/release/inquiry demo and confirm it creates a self-contained HTML report.
6. Test ./target/release/inquiry convert 12 mi km, calculate 'sqrt(2)^2', and one offline research query.
7. Render docs/examples/presidential-milestones-timeline.json to a temporary HTML file and verify that it contains no remote media/data requests and exposes search, filters, citation copy, and CSV export.
8. Test InquiryStudy only with a new synthetic folder created for this verification. Do not inspect my real notes. Build an index, run one cited search, confirm a no-evidence query returns no card, then delete the synthetic index and exports.
9. If this is macOS and I requested the app, run swift test and ./script/build_and_run.sh --verify; do not bypass Gatekeeper or signing protections.
10. If I requested agent integration, configure the MCP server with the absolute release-binary path and args ["mcp"], then call tools/list and the offline convert tool before any live research. Confirm private local-study tools are absent by default and MCP cannot select an arbitrary study directory. Do not set INQUIRY_ENABLE_LOCAL_STUDY_MCP unless I explicitly approve sending returned local excerpts to the MCP host and its model provider.
11. Report what was installed, exact paths/versions/commit, test results, networked sources the live mode can contact, and complete uninstall steps. Stop and ask if any verification fails twice or requires broader authority.
```
