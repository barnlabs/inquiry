#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
cargo build --quiet

RESPONSE="$(target/debug/inquiry mcp --offline <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"inquiry-test","version":"1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"calculate","arguments":{"expression":"sin(pi / 2) + 2^3"}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"resolve_place","arguments":{"query":"White House, Washington DC"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"privacy_check","arguments":{"query":"Research person@example.com"}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"capabilities","arguments":{}}}
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"airport_status","arguments":{"airport":"JFK"}}}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"flight_status_handoff","arguments":{"carrier":"american","flight_identifier":"AA123","date":"2026-07-18"}}}
{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"package_tracking_handoff","arguments":{"carrier":"ups","tracking_identifier":"1Z999AA10123456784"}}}
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"research","arguments":{"query":"dengue disease transmission safety statistics"}}}
EOF
)"
grep -q '"name":"research"' <<<"$RESPONSE"
grep -q '"schema_version":"inquiry.report/v1"' <<<"$RESPONSE"
grep -q 'dengue disease transmission safety statistics' <<<"$RESPONSE"
grep -q '"name":"capabilities"' <<<"$RESPONSE"
grep -q '"name":"airport_status"' <<<"$RESPONSE"
grep -q '"name":"flight_status_handoff"' <<<"$RESPONSE"
grep -q '"name":"package_tracking_handoff"' <<<"$RESPONSE"
grep -q '"name":"graph"' <<<"$RESPONSE"
grep -q '"name":"render_timeline"' <<<"$RESPONSE"
if grep -q '"name":"study_search"' <<<"$RESPONSE"; then
  echo "private local-study MCP tools were enabled without explicit opt-in" >&2
  exit 1
fi
grep -q '"protocolVersion":"2025-11-25"' <<<"$RESPONSE"
grep -q 'Model text is never evidence' <<<"$RESPONSE"
grep -q '"readOnlyHint":true' <<<"$RESPONSE"
grep -q '"openWorldHint":true' <<<"$RESPONSE"
grep -q '"outputSchema":{"type":"object"}' <<<"$RESPONSE"
grep -q '"value":9.0' <<<"$RESPONSE"
grep -q 'place resolution is unavailable in offline mode' <<<"$RESPONSE"
grep -q 'airport status is unavailable offline' <<<"$RESPONSE"
grep -q '"isError":true' <<<"$RESPONSE"
grep -q 'redacted-email' <<<"$RESPONSE"
grep -q '"universal_coverage_claimed":false' <<<"$RESPONSE"
grep -q '"flight_identifier":"AA123"' <<<"$RESPONSE"
grep -q '"tracking_identifier_display":"••••6784"' <<<"$RESPONSE"
grep -q '"identifier_in_url":false' <<<"$RESPONSE"
grep -q '"status_retrieved":false' <<<"$RESPONSE"
if grep -q 'person@example.com' <<<"$RESPONSE"; then
  echo "privacy_check echoed a detected email address" >&2
  exit 1
fi
if grep -q '1Z999AA10123456784' <<<"$RESPONSE"; then
  echo "package handoff echoed a full tracking identifier" >&2
  exit 1
fi
if grep -q '"latitude"' <<<"$RESPONSE"; then
  echo "offline MCP place call leaked network-derived coordinates" >&2
  exit 1
fi

OPT_IN_RESPONSE="$(INQUIRY_ENABLE_LOCAL_STUDY_MCP=1 target/debug/inquiry mcp --offline <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"inquiry-test","version":"1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
EOF
)"
grep -q '"name":"study_search"' <<<"$OPT_IN_RESPONSE"
grep -q '"name":"study_local_pack"' <<<"$OPT_IN_RESPONSE"

PREINIT_RESPONSE="$(target/debug/inquiry mcp --offline <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
EOF
)"
grep -q 'MCP session is not initialized' <<<"$PREINIT_RESPONSE"

LIVE_GATE_RESPONSE="$(target/debug/inquiry mcp <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"inquiry-test","version":"1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"research","arguments":{"query":"Compare GDP and population for Kenya"}}}
EOF
)"
grep -q 'public connector permission is required' <<<"$LIVE_GATE_RESPONSE"

echo "MCP lifecycle, annotations, scoped connector handoffs, private-tool opt-in, calculation, offline research, and live plan gate passed"
