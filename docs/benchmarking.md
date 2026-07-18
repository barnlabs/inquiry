# Benchmarking before claims

Inquiry must request independent benchmarking before anyone claims it reduces model hallucinations or token usage.

The repository also includes a small, transparent release smoke benchmark for high-risk relevance regressions:

```bash
cargo build --release --locked
./script/check_relevance_benchmark.sh
```

This 20-case smoke set checks exact current officeholders, the current UK monarch with authoritative corroboration and rights-checked portrait, jurisdiction ambiguity abstention, U.S. alias routing, three common numbered-president phrasings with wrong-person exclusion, country metrics, exact health-topic selection plus section coverage or explicit abstention, licensed anatomy links, honestly labeled curated open-textbook discovery, scholarly recency filtering, 3D-asset abstention, ambiguous-place handling, privacy failure, multiple medical/emergency and pattern-of-life policy paraphrases, and deterministic math. Assertions inspect outcome properties rather than only checking that a convenient title exists. It is an engineering regression gate, not evidence that Inquiry reduces hallucinations or token usage. Its prompts and assertions are public and therefore unsuitable as a hidden product-quality benchmark.

The initial evaluation should compare:

1. a strong model answering directly;
2. the same model with ordinary web search;
3. the same model using Inquiry's structured report;
4. deterministic Inquiry output without model synthesis.

Use a preregistered, versioned set of at least 300 questions spanning location disambiguation, current news, law/version dates, engineering tolerances, public-health definitions, psychology statistics, chemistry, citations, open-license assets, calculations, and intentionally unanswerable prompts. Keep test answers and adjudication independent from connector implementation.

Report exact models, prompts, tool versions, data cutoff, hardware, latency, peak memory, network calls, input/output tokens, cost, citation correctness, entity-match accuracy, numeric/unit accuracy, abstention quality, unsupported-claim rate, answer usefulness, and failures. Use confidence intervals and paired tests where appropriate. Separate retrieval failures from synthesis failures.

Do not optimize on the hidden evaluation set, average away dangerous category failures, or market a reduction until results reproduce outside BarnLabs. The report should state what the benchmark does not prove.

## Local release budgets

Run:

```bash
./script/benchmark_local.sh
```

The script builds the locked release binary, measures 20 fresh-process invocations per cold-process flow, measures peak resident memory separately with macOS `/usr/bin/time -l`, and sends 100 calls through one initialized offline MCP process for the warm flow. “Cold” means a new Inquiry process, not a cold filesystem cache or post-reboot machine. Timing includes shell launch overhead. The FAA archive case is a synthetic schema/privacy fixture and does not represent a full production archive. Network connectors and UI rendering are excluded.

| Flow | Latency budget | Peak RSS budget | Why this budget exists |
| --- | ---: | ---: | --- |
| Cold `capabilities` | p95 ≤ 150 ms | ≤ 45 MiB | Settings/help should feel immediate |
| Cold scoped-identifier abstention | p95 ≤ 200 ms | ≤ 50 MiB | Sensitive identifiers must fail locally without a perceptible research wait |
| Cold package handoff | p95 ≤ 150 ms | ≤ 45 MiB | The no-network official handoff should feel immediate |
| Cold local aircraft fixture | p95 ≤ 250 ms | ≤ 55 MiB | One-record local import should remain bounded; a full-archive benchmark is still required before a broad claim |
| Warm MCP `capabilities` | mean ≤ 10 ms/call over 100 calls | ≤ 55 MiB | Repeated agent inspection should not create noticeable local overhead |
| Release binary | — | file size ≤ 25 MiB | Keeps installation and bundled-app overhead bounded |

### Verified run — 2026-07-17

Hardware and toolchain: Mac17,2, Apple M5, 32 GiB RAM; macOS 27.0 build 26A5378n; Rust 1.93.1; arm64. Results:

| Flow | Samples | Mean | p95 | Peak RSS | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| Cold `capabilities` | 20 | 23.236 ms | 11.270 ms | 6.984 MiB | Pass |
| Cold scoped abstention | 20 | 16.917 ms | 28.782 ms | 10.453 MiB | Pass |
| Cold package handoff | 20 | 10.515 ms | 13.192 ms | 7.172 MiB | Pass |
| Cold local aircraft fixture | 20 | 10.401 ms | 10.931 ms | 7.750 MiB | Pass |
| Warm MCP `capabilities` | 100 | 0.145 ms/call | not separately sampled | 8.531 MiB | Pass |
| Release binary | 1 | 7.756 MiB | — | — | Pass |

The cold-capability mean exceeds its p95 because one cache/startup outlier is excluded by the nearest-rank p95 for 20 samples; raw samples are intentionally temporary and the release record does not claim a population distribution from this small run. Re-run the script after dependency, linker, connector-routing, or serialization changes. These results do not establish network latency, app launch time, energy use, or model/token improvements.
