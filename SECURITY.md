# Security policy

## Supported versions

Inquiry is pre-1.0. Security fixes target the latest main branch until tagged releases begin.

## Reporting

Do not open a public issue for a vulnerability involving code execution, credential exposure, private data, or a bypass of the sensitive-targeting policy. Use GitHub's private security advisory flow for the repository owner. Do not include real private-person, patient, or confidential-investigation data in a report.

## Security model

Inquiry makes outbound HTTPS requests only to configured or documented public connectors, writes reports to caller-selected local paths, and exposes MCP tools over stdio. The only plaintext exception is an explicitly configured loopback SearXNG endpoint for local development. SearXNG validation rejects embedded credentials, non-loopback HTTP, literal private/link-local IPs, and `.local` or `.internal` names; it does not provide rebinding-proof DNS pinning, so the user-supplied endpoint remains trusted configuration. It does not run fetched scripts, execute page content, access browser sessions, or require secrets for the current connectors. SearXNG is opt-in through `INQUIRY_SEARXNG_URL`. openFDA is used without an API key and remains subject to its unauthenticated service limits.

Reports are untrusted-data renderings. Connector text is escaped. Source URLs are restricted to HTTP(S) for discovery results, and the shared structured-connector client does not follow redirects; a redirect is surfaced as a connector failure rather than silently contacting another destination. Generated HTML never loads remote media automatically. The native app can load an exact cited portrait when its separate preference is on: only `https://upload.wikimedia.org` is accepted, cross-host redirects are rejected, cookies and credential storage are disabled, transfers are capped at 5 MB, ImageIO validates one frame and bounded dimensions before decoding, and the decoded thumbnail's aspect ratio must match the cited Commons metadata. All other previews require a user click. Users should still open unknown links cautiously and review generated reports before sharing them.

InquiryStudy treats every local filename and document as hostile input. It accepts a bounded allowlist, rejects symlinks and special files, checks common signatures, applies file/corpus/page/archive/XML/segment limits, refuses encrypted PDFs and macro-bearing Office packages, rejects document types/entities anywhere in Office XML, excludes script/style HTML, and does not extract ZIP contents to disk. Private indexes and recall exports are create-new owner-only files. Imported index fields are bounded and self-consistency checked, but their checksums are not signatures and do not authenticate a forged index against the original files.

Local-study MCP tools are disabled unless the operator explicitly sets `INQUIRY_ENABLE_LOCAL_STUDY_MCP=1`. When enabled, the MCP host and its model provider may receive private excerpts even though Inquiry itself makes no connector request. The MCP surface cannot create an index or access arbitrary paths; it can only search a named index under a real, non-symlink `reports/` directory. There remains a narrow local directory-substitution race because the current implementation does not retain a directory file descriptor for every handle-relative operation.

These controls reduce exposure but do not prove parser safety. The current PDF and Office parsers run in the Inquiry process rather than a separate sandbox. PDF text extraction can consume substantial CPU or memory before the later aggregate text budget is applied. Report malformed-document crashes privately, use synthetic fixtures for reproduction, and do not index untrusted downloads on a high-value system until out-of-process isolation, strict wall-clock/memory budgets, and fuzzing are complete.
