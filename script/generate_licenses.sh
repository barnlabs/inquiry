#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_ABOUT="$ROOT_DIR/.tools/bin/cargo-about"

if [[ ! -x "$CARGO_ABOUT" ]]; then
  echo "cargo-about is missing; install it locally with: cargo install cargo-about --locked --root .tools --features cli" >&2
  exit 1
fi

cd "$ROOT_DIR"
"$CARGO_ABOUT" generate about.hbs --output-file THIRD_PARTY_LICENSES.html
test -s THIRD_PARTY_LICENSES.html
echo "$ROOT_DIR/THIRD_PARTY_LICENSES.html"
