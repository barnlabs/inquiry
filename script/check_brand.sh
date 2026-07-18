#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BRAND_DIR="$ROOT_DIR/brand"
TASK_TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/inquiry-brand-check.XXXXXX")"
trap 'rm -rf "$TASK_TMP_DIR"' EXIT

command -v magick >/dev/null || {
  echo "ImageMagick is required for brand verification" >&2
  exit 2
}
command -v identify >/dev/null || {
  echo "ImageMagick identify is required for brand verification" >&2
  exit 2
}

for file in \
  inquiry-mark.svg \
  inquiry-mark-small.svg \
  inquiry-app-icon.svg \
  inquiry-mark-mono-white.svg \
  inquiry-mark-mono-teal.svg \
  inquiry-wordmark.svg \
  inquiry-og.svg; do
  test -s "$BRAND_DIR/$file" || { echo "missing $file" >&2; exit 1; }
  rg -q '<title[^>]*>' "$BRAND_DIR/$file" || { echo "$file has no accessible title" >&2; exit 1; }
  rg -q '<desc[^>]*>' "$BRAND_DIR/$file" || { echo "$file has no accessible description" >&2; exit 1; }
done

for size in 16 32; do
  magick -background none "$BRAND_DIR/inquiry-mark-small.svg" -resize "${size}x${size}" "$TASK_TMP_DIR/mark-$size.png"
done
for size in 64 128 256 512 1024; do
  magick -background none "$BRAND_DIR/inquiry-mark.svg" -resize "${size}x${size}" "$TASK_TMP_DIR/mark-$size.png"
done
for size in 16 32 64 128 256 512 1024; do
  test "$(identify -format '%wx%h' "$TASK_TMP_DIR/mark-$size.png")" = "${size}x${size}" || {
    echo "mark-$size.png has the wrong dimensions" >&2
    exit 1
  }
  colors="$(identify -format '%k' "$TASK_TMP_DIR/mark-$size.png")"
  test "$colors" -ge 4 || { echo "mark-$size.png lost essential color separation" >&2; exit 1; }
done

magick -background '#0a1f1a' "$BRAND_DIR/inquiry-app-icon.svg" -resize 1024x1024 -alpha off PNG24:"$TASK_TMP_DIR/app-icon.png"
test "$(identify -format '%wx%h' "$TASK_TMP_DIR/app-icon.png")" = "1024x1024"
if identify -format '%[channels]' "$TASK_TMP_DIR/app-icon.png" | grep -qi 'a'; then
  echo "macOS app icon unexpectedly contains an alpha channel" >&2
  exit 1
fi

contrast_ratio() {
  local foreground="${1#\#}"
  local background="${2#\#}"
  local fr=$((16#${foreground:0:2}))
  local fg=$((16#${foreground:2:2}))
  local fb=$((16#${foreground:4:2}))
  local br=$((16#${background:0:2}))
  local bg=$((16#${background:2:2}))
  local bb=$((16#${background:4:2}))
  awk -v fr="$fr" -v fg="$fg" -v fb="$fb" -v br="$br" -v bg="$bg" -v bb="$bb" '
    function linear(value) {
      value /= 255
      return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ^ 2.4
    }
    function luminance(r, g, b) { return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b) }
    BEGIN {
      one = luminance(fr, fg, fb)
      two = luminance(br, bg, bb)
      high = one > two ? one : two
      low = one > two ? two : one
      printf "%.3f", (high + 0.05) / (low + 0.05)
    }
  '
}

text_ratio="$(contrast_ratio '#e8f5ee' '#0a1f1a')"
teal_ratio="$(contrast_ratio '#2dd4a7' '#0a1f1a')"
gold_ratio="$(contrast_ratio '#f5c842' '#0a1f1a')"
awk -v value="$text_ratio" 'BEGIN { exit !(value >= 4.5) }'
awk -v value="$teal_ratio" 'BEGIN { exit !(value >= 3.0) }'
awk -v value="$gold_ratio" 'BEGIN { exit !(value >= 3.0) }'

echo "Brand checks passed: 16-1024 px exports, accessible SVG metadata, opaque app icon, and contrast ratios text=$text_ratio teal=$teal_ratio gold=$gold_ratio"
