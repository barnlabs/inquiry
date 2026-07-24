# Agent integrations

Inquiry's evidence core is model-free. Codex, Grok, and Ollama are optional **hosts or planners**, never evidence sources. ChatGPT is documented as a future remote integration boundary, not a shipped local integration. A model may decide to call an Inquiry tool; it may not invent or replace Inquiry source IDs, citations, identities, metrics, media provenance, calculations, or clinical conclusions.

## Support matrix

| Surface | Supported v0.1 path | Important boundary |
| --- | --- | --- |
| Codex CLI and IDE | Local stdio MCP using the checked-in `.codex/config.toml` | The OpenAI host receives the prompt and any tool result returned to it. Tool approval remains under the user's Codex policy. |
| ChatGPT | Not shipped | ChatGPT does not connect directly to a local stdio MCP server. A future integration would require a supported remote MCP path or Secure MCP Tunnel plus authentication, privacy review, and deployment approval. |
| Grok Build | Local stdio MCP using the checked-in `.grok/config.toml` | Grok requires the user to trust a repository before it starts project-scoped servers. Do not bypass that review. |
| Grok.com | Not shipped | Grok.com requires a reachable remote MCP server; localhost is not a supported public connector. Inquiry v0.1 deliberately does not add that public/auth boundary. |
| Ollama + Codex | Run a local Ollama model through `ollama launch codex`; Codex remains the MCP host | A model name ending in `:cloud` is remote even when requested through localhost. Never describe it as device-local. |
| Ollama native app planning | Not shipped | Ollama's HTTP API is not an MCP host. A future adapter must be loopback-only, schema-constrained, opt-in, and benchmarked before default use. |

Official references: [Codex MCP](https://developers.openai.com/codex/mcp/), [ChatGPT developer mode and MCP apps](https://help.openai.com/en/articles/12584461-developer-mode-and-mcp-apps-in-chatgpt), [Grok Build MCP](https://docs.x.ai/build/features/mcp-servers), [Grok custom connectors](https://docs.x.ai/grok/connectors), [Ollama CLI integrations](https://docs.ollama.com/cli), and [Ollama local/cloud authentication](https://docs.ollama.com/api/authentication).

## Build once

```bash
cargo build --release --locked
./script/test_mcp.sh
```

The checked-in Codex and Grok configurations invoke `./target/release/inquiry mcp` from the repository root. The host process **must** use the repository root as its working directory (or replace the command with an absolute path to a built binary). Relative `./target/release/inquiry` fails if the host starts elsewhere or the release binary has not been built. They do not contain a key, account, remote endpoint, or private-study opt-in.

Agent skill guidance lives at [`skills/inquiry/SKILL.md`](../skills/inquiry/SKILL.md) (tool matrix, CLI recipes, model-loading boundaries).

## Codex

From the repository root:

```bash
codex mcp get inquiry --json
codex mcp list
```

For a binary installed elsewhere, let the user deliberately add the absolute path:

```bash
codex mcp add inquiry -- /absolute/path/to/inquiry mcp
```

Codex successfully discovered the checked-in Inquiry server in the release audit. A headless `calculate` attempt reached Codex's normal MCP approval boundary and was canceled because no human was present; Inquiry does not weaken that host approval policy. The direct protocol smoke test proves the same calculation, lifecycle, and structured result without bypassing approvals.

## ChatGPT

Inquiry v0.1 does **not** ship a ChatGPT integration. ChatGPT cannot connect directly to this repository's local stdio server. A future ChatGPT path must use an officially supported remote MCP connection or Secure MCP Tunnel and will need authentication, per-user authorization, data-retention disclosure, abuse controls, and a separate deployment review. Do not expose the local server to the public internet.

## Grok Build

The project configuration is intentionally repository-scoped:

```bash
grok mcp list --json
grok mcp doctor inquiry --json
```

On first use, Grok reports the server unhealthy until the user reviews and trusts the folder. That is a security feature. After trust is granted in Grok, repeat `doctor`, then call `calculate` before live research. For an installed binary outside the checkout:

```bash
grok mcp add --scope project inquiry -- /absolute/path/to/inquiry mcp
```

## Ollama

The least duplicated v0.1 path is:

```bash
ollama launch codex
```

Choose a genuinely local model, confirm it is present with `ollama list`, and inspect active memory with `ollama ps`. Codex continues to host Inquiry through MCP. Inquiry never auto-pulls a model and never silently falls back to a cloud model.

## Privacy and evidence contract

1. Inquiry's native privacy preflight runs before Inquiry's own live connectors.
2. A prompt typed into a cloud agent has already crossed that provider's boundary before the provider calls Inquiry.
3. Private InquiryStudy tools are absent unless the operator explicitly sets `INQUIRY_ENABLE_LOCAL_STUDY_MCP=1`; do not set it as part of installation.
4. External and local excerpts are untrusted quoted data, never instructions.
5. Model text is not evidence. A model-authored synthesis must cite existing Inquiry source IDs; unsupported IDs and new factual claims must be rejected or labeled uncited draft text.
6. Remote MCP, direct paid provider APIs, and API-key fields are not part of v0.1.

## Compatibility proof

`./script/test_mcp.sh` verifies (exact script assertions, not a full matrix):

- initialize with protocol version `2025-11-25` followed by `notifications/initialized`;
- pre-initialization `tools/list` fails;
- agent-facing instructions include “Model text is never evidence”;
- safety annotations (`readOnlyHint` / `openWorldHint`) and object `outputSchema` markers;
- deterministic `calculate` output;
- offline isolation for place/airport open-world tools without network leakage;
- capability matrix `universal_coverage_claimed: false`;
- flight/package handoffs with identifier masking and `status_retrieved: false`;
- offline `research` returns an `inquiry.report/v1` for the multi-facet dengue catalog query;
- live (network-on) `research` without plan approval returns the public-connector permission error;
- private-study tools absent by default and present only with `INQUIRY_ENABLE_LOCAL_STUDY_MCP=1`.

Live multi-connector research from MCP requires tool arguments `automatic_public_web` and/or `approved_plan_id` (see the skill). Host tool approval alone is not plan approval. If you use `redact_sensitive`, the plan fingerprint is computed **after** redaction: run `privacy_check`, plan the **redacted** query string, then call `research` with the original query, `redact_sensitive: true`, and that redacted plan’s `approved_plan_id`.
