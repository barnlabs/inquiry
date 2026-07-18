#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${INQUIRY_BIN:-$ROOT_DIR/target/release/inquiry}"
TASK_TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/inquiry-connector-check.XXXXXX")"
trap 'rm -rf "$TASK_TMP_DIR"' EXIT

command -v jq >/dev/null || { echo "jq is required" >&2; exit 2; }
test -x "$BIN" || cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --release --locked --quiet

"$BIN" capabilities >"$TASK_TMP_DIR/capabilities.json"
jq -e '.universal_coverage_claimed == false
  and ([.capabilities[].id] | index("faa-airport-status") != null)
  and ([.capabilities[].id] | index("package-tracking") != null)
  and ([.capabilities[].id] | index("chatgpt-direct-local-mcp") != null)' \
  "$TASK_TMP_DIR/capabilities.json" >/dev/null

"$BIN" research 'track UPS 1Z999AA10123456784' --format json >"$TASK_TMP_DIR/guard.json"
jq -e '.run.network_used == false
  and (.run.connectors_attempted | length) == 0
  and (.sources | length) == 0
  and (.findings | length) == 0
  and (.summary | contains("did not send the package identifier"))' \
  "$TASK_TMP_DIR/guard.json" >/dev/null
if rg -q '1Z999AA10123456784' "$TASK_TMP_DIR/guard.json"; then
  echo "scoped abstention report retained a full tracking identifier" >&2
  exit 1
fi

for separated_identifier in '1Z-999-AA10-1234-5678-4' '1Z 999 AA10 1234 5678 4'; do
  "$BIN" research "track UPS $separated_identifier" --format json >"$TASK_TMP_DIR/guard-separated.json"
  jq -e '.run.network_used == false
    and (.run.connectors_attempted | length) == 0
    and (.sources | length) == 0
    and (.findings | length) == 0
    and (.summary | contains("did not send the package identifier"))' \
    "$TASK_TMP_DIR/guard-separated.json" >/dev/null
  if rg -Fq "$separated_identifier" "$TASK_TMP_DIR/guard-separated.json"; then
    echo "scoped abstention report retained a separated tracking identifier" >&2
    exit 1
  fi
done

"$BIN" flight-status american AA123 --date 2026-07-18 >"$TASK_TMP_DIR/flight.json"
jq -e '.carrier == "American Airlines"
  and .flight_identifier == "AA123"
  and .status_retrieved == false
  and .network_used == false
  and (.official_status_page | startswith("https://www.aa.com/"))' \
  "$TASK_TMP_DIR/flight.json" >/dev/null

printf '%s' '1Z999AA10123456784' | "$BIN" package-tracking ups --stdin >"$TASK_TMP_DIR/package.json"
jq -e '.carrier == "UPS"
  and .tracking_identifier_display == "••••6784"
  and .identifier_in_url == false
  and .status_retrieved == false
  and .network_used == false
  and .official_tracking_url == "https://www.ups.com/track?loc=en_US"' \
  "$TASK_TMP_DIR/package.json" >/dev/null
if rg -q '1Z999AA10123456784' "$TASK_TMP_DIR/package.json"; then
  echo "default package handoff retained a full tracking identifier" >&2
  exit 1
fi

"$BIN" package-tracking ups 1Z999AA10123456784 --deep-link >"$TASK_TMP_DIR/package-deep-link.json"
jq -e '.identifier_in_url == true
  and .status_retrieved == false
  and ((.official_tracking_url | fromjson? // .) | startswith("https://www.ups.com/track?"))
  and (.official_tracking_url | contains("tracknum=1Z999AA10123456784"))' \
  "$TASK_TMP_DIR/package-deep-link.json" >/dev/null

if "$BIN" airport-status JFK --offline >"$TASK_TMP_DIR/airport-offline.json" 2>"$TASK_TMP_DIR/airport-offline.err"; then
  echo "offline airport status unexpectedly succeeded" >&2
  exit 1
fi
rg -q 'Inquiry did not contact the FAA' "$TASK_TMP_DIR/airport-offline.err"

if test "${INQUIRY_REQUIRE_LIVE_FAA:-0}" = 1; then
  "$BIN" airport-status JFK >"$TASK_TMP_DIR/airport-live.json"
  jq -e '.airport_id == "JFK"
    and .network_used == true
    and .retrieved_at != null
    and .source_updated_at != null
    and .source_url == "https://nasstatus.faa.gov/api/airport-events"
    and (.warning | contains("not suitable for navigation"))' \
    "$TASK_TMP_DIR/airport-live.json" >/dev/null
  printf 'PASS live FAA airport status: %s event(s), %s ms\n' \
    "$(jq '.active_events | length' "$TASK_TMP_DIR/airport-live.json")" \
    "$(jq '.latency_ms' "$TASK_TMP_DIR/airport-live.json")"
else
  echo "SKIP live FAA airport status (set INQUIRY_REQUIRE_LIVE_FAA=1)"
fi

echo "Scoped coverage, identifier guard, airline/package handoffs, and offline isolation passed"
