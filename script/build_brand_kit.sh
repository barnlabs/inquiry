#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BRAND="$ROOT/brand"

command -v magick >/dev/null || {
  echo "ImageMagick is required to regenerate Inquiry raster marks" >&2
  exit 2
}

for size in 16 32; do
  magick -background none "$BRAND/inquiry-mark-small.svg" -resize "${size}x${size}" "$BRAND/inquiry-mark-${size}.png"
done
for size in 64 128 256 512 1024; do
  magick -background none "$BRAND/inquiry-mark.svg" -resize "${size}x${size}" "$BRAND/inquiry-mark-${size}.png"
done

magick -background none "$BRAND/inquiry-wordmark.svg" -resize 1040x "$BRAND/inquiry-wordmark-1040.png"
magick -background none "$BRAND/inquiry-og.svg" -resize '1200x630!' "$BRAND/inquiry-og.png"
magick -background '#0a1f1a' "$BRAND/inquiry-app-icon.svg" -resize 512x512 -alpha off PNG24:"$ROOT/macos/Resources/Inquiry.png"

assets=(
  README.md
  TRADEMARKS.md
  inquiry-mark.svg
  inquiry-mark-small.svg
  inquiry-app-icon.svg
  inquiry-wordmark.svg
  inquiry-mark-mono-white.svg
  inquiry-mark-mono-teal.svg
  inquiry-mark-16.png
  inquiry-mark-32.png
  inquiry-mark-64.png
  inquiry-mark-128.png
  inquiry-mark-256.png
  inquiry-mark-512.png
  inquiry-mark-1024.png
  inquiry-wordmark-1040.png
  inquiry-og.svg
  inquiry-og.png
)

(
  cd "$BRAND"
  shasum -a 256 "${assets[@]}" > SHA256SUMS
  rm -f inquiry-brand-kit.zip inquiry-brand-kit.zip.sha256
  zip -X -q inquiry-brand-kit.zip "${assets[@]}" SHA256SUMS
  shasum -a 256 inquiry-brand-kit.zip > inquiry-brand-kit.zip.sha256
)

echo "Built $BRAND/inquiry-brand-kit.zip"
