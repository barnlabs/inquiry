#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT_DIR/target/release/inquiry"
TASK_TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/inquiry-local-benchmark.XXXXXX")"
trap 'rm -rf "$TASK_TMP_DIR"' EXIT

command -v perl >/dev/null || { echo "perl with Time::HiRes is required" >&2; exit 2; }
command -v zip >/dev/null || { echo "zip is required" >&2; exit 2; }

cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --release --locked --quiet

now_ms() {
  perl -MTime::HiRes=time -e 'printf "%.3f", time * 1000'
}

measure_flow() {
  local name="$1"
  local latency_budget_ms="$2"
  local rss_budget_mib="$3"
  shift 3
  local samples="$TASK_TMP_DIR/$name.samples"
  : >"$samples"
  local start end elapsed
  for _ in $(seq 1 20); do
    start="$(now_ms)"
    "$@" >/dev/null
    end="$(now_ms)"
    elapsed="$(awk -v start="$start" -v end="$end" 'BEGIN { printf "%.3f", end - start }')"
    printf '%s\n' "$elapsed" >>"$samples"
  done
  local mean p95
  mean="$(awk '{ total += $1 } END { printf "%.3f", total / NR }' "$samples")"
  p95="$(sort -n "$samples" | awk 'NR == 19 { printf "%.3f", $1 }')"

  local time_output="$TASK_TMP_DIR/$name.time"
  /usr/bin/time -l -o "$time_output" "$@" >/dev/null
  local rss_bytes rss_mib
  rss_bytes="$(awk '/maximum resident set size/ { print $1 }' "$time_output")"
  rss_mib="$(awk -v bytes="$rss_bytes" 'BEGIN { printf "%.3f", bytes / 1048576 }')"

  local pass=true
  if ! awk -v actual="$p95" -v budget="$latency_budget_ms" 'BEGIN { exit !(actual <= budget) }'; then
    pass=false
  fi
  if ! awk -v actual="$rss_mib" -v budget="$rss_budget_mib" 'BEGIN { exit !(actual <= budget) }'; then
    pass=false
  fi
  printf '{"flow":"%s","samples":20,"mean_ms":%s,"p95_ms":%s,"latency_budget_ms":%s,"peak_rss_mib":%s,"rss_budget_mib":%s,"pass":%s}\n' \
    "$name" "$mean" "$p95" "$latency_budget_ms" "$rss_mib" "$rss_budget_mib" "$pass"
  test "$pass" = true
}

printf '%s\n' 'N-NUMBER,SERIAL NUMBER,MFR MDL CODE,YEAR MFR,TYPE AIRCRAFT,TYPE ENGINE,STATUS CODE,CERT ISSUE DATE,EXPIRATION DATE,LAST ACTIVITY DATE,NAME,STREET1,MODE S CODE HEX' \
  '123AB,SECRET-SERIAL,ABC1234,2020,4,1,V,20200101,20270101,20260701,PRIVATE PERSON,123 PRIVATE STREET,A00001' \
  >"$TASK_TMP_DIR/MASTER.txt"
printf '%s\n' 'CODE,MFR,MODEL,TYPE-ACFT,TYPE-ENG' \
  'ABC1234,BARN AIRCRAFT,MODEL ONE,4,1' \
  >"$TASK_TMP_DIR/ACFTREF.txt"
(
  cd "$TASK_TMP_DIR"
  zip -q faa-fixture.zip MASTER.txt ACFTREF.txt
)

measure_flow cold_capabilities 150 45 "$BIN" capabilities
measure_flow cold_scoped_abstention 200 50 "$BIN" research 'track UPS 1Z999AA10123456784' --format json
measure_flow cold_package_handoff 150 45 "$BIN" package-tracking ups 1Z999AA10123456784
measure_flow cold_aircraft_fixture 250 55 "$BIN" aircraft-lookup N123AB --registry "$TASK_TMP_DIR/faa-fixture.zip"

REQUESTS="$TASK_TMP_DIR/mcp-requests.jsonl"
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"inquiry-benchmark","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  >"$REQUESTS"
for identifier in $(seq 2 101); do
  printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"capabilities","arguments":{}}}\n' "$identifier" >>"$REQUESTS"
done

start="$(now_ms)"
"$BIN" mcp --offline <"$REQUESTS" >/dev/null
end="$(now_ms)"
warm_average="$(awk -v start="$start" -v end="$end" 'BEGIN { printf "%.3f", (end - start) / 100 }')"
time_output="$TASK_TMP_DIR/warm_mcp.time"
/usr/bin/time -l -o "$time_output" "$BIN" mcp --offline <"$REQUESTS" >/dev/null
rss_bytes="$(awk '/maximum resident set size/ { print $1 }' "$time_output")"
rss_mib="$(awk -v bytes="$rss_bytes" 'BEGIN { printf "%.3f", bytes / 1048576 }')"
warm_pass=true
if ! awk -v actual="$warm_average" 'BEGIN { exit !(actual <= 10) }'; then warm_pass=false; fi
if ! awk -v actual="$rss_mib" 'BEGIN { exit !(actual <= 55) }'; then warm_pass=false; fi
printf '{"flow":"warm_mcp_capabilities","samples":100,"mean_ms":%s,"latency_budget_ms":10,"peak_rss_mib":%s,"rss_budget_mib":55,"pass":%s}\n' \
  "$warm_average" "$rss_mib" "$warm_pass"
test "$warm_pass" = true

binary_bytes="$(stat -f '%z' "$BIN")"
binary_mib="$(awk -v bytes="$binary_bytes" 'BEGIN { printf "%.3f", bytes / 1048576 }')"
binary_pass=true
if ! awk -v actual="$binary_mib" 'BEGIN { exit !(actual <= 25) }'; then binary_pass=false; fi
printf '{"flow":"release_binary","size_mib":%s,"size_budget_mib":25,"pass":%s}\n' "$binary_mib" "$binary_pass"
test "$binary_pass" = true
