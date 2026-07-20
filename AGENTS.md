# Inquiry contributor instructions

Inquiry is an early local-first, source-grounded developer preview. Read `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `docs/architecture.md`, `docs/privacy.md`, and the task-relevant source documentation before editing.

## Give a lower-capability agent one bounded task

The task must name one outcome, exact files, invariants, proof commands, reviewer, and stop boundary. Good tasks include one connector fixture, one deterministic relevance case, one privacy regression, one parser limit, one accessible UI state, one packaging assertion, or one documentation correction. Split broad requests.

Do not delegate authenticated scraping, person-level scoring, de-anonymization, medical diagnosis/dosing, autonomous transactions, release signing, account access, private-data migration, or production publication to a low-capability agent.

## Invariants

- Preserve local-first, source-grounded behavior. Connector responses, files, indexes, URLs, model text, discovery records, and report inputs are untrusted data.
- `ContentTrust::ExternalUntrusted` is a data-origin/security boundary, not a claim that a publisher is unreliable. Citation presence is not truth.
- Resolve the exact entity, jurisdiction, metric, unit, period, and uncertainty. Ambiguity must abstain or ask; it must not silently choose a person or place.
- Preserve source URL, retrieval time, license/terms, transformation, and limitations. Never invent citations or imply that discovery-only records were verified.
- Do not add authenticated scraping, silent redirects, automatic remote media, unbounded retries/polling, full tracking identifiers in normal output/URLs, or high-risk person-location/pattern-of-life behavior.
- Keep local files hostile: reject symlinks and special files where required, bound size/time/memory, escape rendered content, minimize private indexes/exports, and use synthetic fixtures.
- Do not commit secrets, real private data, browser/session material, credentials, or copied restricted content/assets.

## Simple work loop

1. Inspect the named source, tests, and current state. Git metadata currently may return `Operation not permitted`; do not work around it or claim a clean branch.
2. Add one failing deterministic test or fixture that states the expected behavior.
3. Implement the smallest change.
4. Run the focused command, then the applicable checks below.
5. Inspect the diff for wrong-entity matches, privacy leaks, redirect/retry amplification, parser escape, provenance/license loss, and unsupported claims.
6. Update every affected README, architecture/privacy/capability document, release checklist, or GitHub issue/PR record in the same change; record exact commands and remaining limits.
7. Give **two fresh, context-isolated adversarial reviewers** the task contract, diff, fixtures, evidence, and failure questions—not the implementer's conclusion. Each reopens source and tests, reruns the smallest relevant check, and probes a plausible wrong path.
8. Resolve or explicitly record both review findings, then return PASS or REWORK. Stop on missing source terms, ambiguous entity/privacy behavior, non-determinism, permission failure, or expanded scope.

## Verified command menu

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
swift test
./script/test_mcp.sh
./script/check_brand.sh
cargo build --release --locked
./script/check_relevance_benchmark.sh
./script/check_connector_flows.sh
./script/benchmark_local.sh
./script/build_and_run.sh --verify
./script/build_mcpb.sh
node script/test_timeline_ui.cjs <timeline.html>
```

Run only commands relevant to the task, then expand to the release gate when requested. Dependency installation, license generation, live connector checks, signing, notarization, release, and external publication are separate owner-authorized actions.

## Independent reviewer

For substantive work, **two fresh, context-isolated adversarial reviewers** each reopen source and tests and check at least one plausible failure: wrong person/entity, jurisdiction ambiguity, stale version/date, unit or period mismatch, redaction leak, redirect, oversized/malformed response, parser timeout/crash, retry storm, license/provenance loss, inaccessible state, or extra package contents. Findings include severity, confidence, preconditions, impact, evidence, fix, verification, and remaining risk.

## Git and GitHub flow

Do not run write commands while `.git/config` is permission-blocked, the tree is not understood, or the owner has not authorized the branch/push/PR.

Before any remote mutation, verify the canonical repository with `gh repo view barnlabs/inquiry --json nameWithOwner,visibility,isPrivate,url,defaultBranchRef`. Record the result in the PR. Preserve the repository's current visibility unless Donovan explicitly names a different target. GitHub housekeeping changes need a bounded reviewed task covering `CODEOWNERS`, PR/issue templates, Actions permissions and pinned actions, required checks/branch protection or rulesets, dependency alerts, secret-scanning availability, merge methods, branch deletion, releases, environments, and least-privilege collaborators. Two fresh context-isolated reviewers inspect the settings proposal; the implementer cannot self-approve or self-merge.

```bash
git status --short --branch
git fetch --prune
git switch main
git pull --ff-only
git switch -c contrib/<task-slug>

git diff --check
git diff -- <exact-files>
git add -- <exact-files>
git diff --cached --check
git diff --cached
git commit -m "<area>: <bounded change>"
git push -u origin HEAD
gh pr create --draft --fill
```

Pull only from a clean worktree. Never use destructive reset/checkout, `git add .` in a dirty tree, force-push, push to a protected branch, merge, tag, release, change visibility, or mutate GitHub settings/secrets. Two fresh context-isolated reviewers approve the diff; a different authorized human decides when the PR becomes ready and merges it. The implementer must not merge its own PR, even when the implementer also owns the repository.

## Stop and ask

Ask before dependencies/manifests, new network providers, authentication, private data, weakened abstention/provenance/privacy, signing/notarization, GitHub Actions secrets, release assets, deployment, destructive work, or any public claim. If Git metadata remains unreadable, leave Git state unverified and hand the file diff to the owner.
