#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STAGE_DIR="$ROOT_DIR/target/mcpb-stage"
OUT_DIR="$ROOT_DIR/dist"

cd "$ROOT_DIR"
command -v jq >/dev/null || { echo "jq is required to verify the MCPB tool manifest" >&2; exit 1; }
test -s THIRD_PARTY_LICENSES.html || { echo "run ./script/generate_licenses.sh first" >&2; exit 1; }
cargo build --release --locked
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/server" "$OUT_DIR"
rm -f "$OUT_DIR/barnlabs-inquiry-darwin-arm64.mcpb"
cp packaging/mcpb/manifest.json "$STAGE_DIR/manifest.json"
cp LICENSE NOTICE THIRD_PARTY_LICENSES.html "$STAGE_DIR/"
cp target/release/inquiry "$STAGE_DIR/server/inquiry"
chmod +x "$STAGE_DIR/server/inquiry"

cd "$STAGE_DIR"
/usr/bin/zip -X -q "$OUT_DIR/barnlabs-inquiry-darwin-arm64.mcpb" manifest.json LICENSE NOTICE THIRD_PARTY_LICENSES.html server/inquiry
EXPECTED_ENTRIES=$'LICENSE\nNOTICE\nTHIRD_PARTY_LICENSES.html\nmanifest.json\nserver/inquiry'
ACTUAL_ENTRIES="$(/usr/bin/unzip -Z1 "$OUT_DIR/barnlabs-inquiry-darwin-arm64.mcpb" | LC_ALL=C sort)"
test "$ACTUAL_ENTRIES" = "$EXPECTED_ENTRIES" || {
  echo "unexpected MCPB entry list" >&2
  printf '%s\n' "$ACTUAL_ENTRIES" >&2
  exit 1
}
ARCHIVE_HASH="$(/usr/bin/unzip -p "$OUT_DIR/barnlabs-inquiry-darwin-arm64.mcpb" server/inquiry | shasum -a 256 | awk '{print $1}')"
SOURCE_HASH="$(shasum -a 256 "$STAGE_DIR/server/inquiry" | awk '{print $1}')"
test "$ARCHIVE_HASH" = "$SOURCE_HASH" || { echo "MCPB binary hash mismatch" >&2; exit 1; }

MANIFEST_TOOLS="$(jq -r '.tools[].name' "$STAGE_DIR/manifest.json" | LC_ALL=C sort)"
RUNTIME_TOOLS="$(printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"mcpb-package-check","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  | INQUIRY_ENABLE_LOCAL_STUDY_MCP=1 "$STAGE_DIR/server/inquiry" mcp --offline \
  | jq -r 'select(.id == 2) | .result.tools[].name' \
  | LC_ALL=C sort)"
test "$MANIFEST_TOOLS" = "$RUNTIME_TOOLS" || {
  echo "MCPB manifest tool declarations differ from runtime tools/list" >&2
  diff -u <(printf '%s\n' "$MANIFEST_TOOLS") <(printf '%s\n' "$RUNTIME_TOOLS") >&2 || true
  exit 1
}
(cd "$OUT_DIR" && shasum -a 256 barnlabs-inquiry-darwin-arm64.mcpb > barnlabs-inquiry-darwin-arm64.mcpb.sha256)
echo "$OUT_DIR/barnlabs-inquiry-darwin-arm64.mcpb"
