# BarnLabs Inquiry brand kit

Inquiry's **Focus Frame** mark is a bounded research surface containing three evidence lines and one gold cited point. The open corners mean that inquiry continues; the frame means the released result stays within accepted evidence. The identity is designed to feel technical, humane, and auditable rather than magical, omniscient, or surveillance-oriented.

The public name lockup is **BarnLabs Inquiry**. “Inquiry” is an ordinary descriptive word used by unrelated research and education products, so the BarnLabs pairing reduces avoidable confusion. A preliminary web and USPTO search informed this choice but is not a legal clearance opinion; BarnLabs should obtain qualified trademark review before a major commercial launch or filing.

## Downloadable assets

- `inquiry-mark.svg` — canonical full-color vector mark;
- `inquiry-mark-small.svg` — optically simplified source for 16 px and 32 px exports;
- `inquiry-app-icon.svg` — opaque square source for system-applied macOS icon masking;
- `inquiry-wordmark.svg` — horizontal “BarnLabs Inquiry” lockup on its canonical Ink background, safe on light or dark pages;
- `inquiry-mark-mono-white.svg` and `inquiry-mark-mono-teal.svg` — one-color variants;
- `inquiry-mark-{16,32,64,128,256,512,1024}.png` — transparent raster sizes;
- `inquiry-wordmark-1040.png` — raster wordmark;
- `inquiry-og.svg` and `inquiry-og.png` — 1200 × 630 social artwork;
- `inquiry-brand-kit.zip` — portable bundle of the public assets and this guide;
- `SHA256SUMS` — checksums for release and cache verification.

The SVG files are canonical. Prefer them for websites, documentation, print, and any new raster export. The 16 px and 32 px PNGs intentionally use the small optical source; do not recreate them by shrinking the full mark.

## Color system

| Token | Hex | Role |
| --- | --- | --- |
| Ink | `#0A1F1A` | deepest outlines and dark background |
| Forest surface | `#102A22` | mark field and product surfaces |
| Evidence teal | `#2DD4A7` | discovery, links, active evidence |
| Source gold | `#F5C842` | cited source and lens handle |
| Evidence mist | `#B8D8CD` | source lines and secondary data |
| Frame sage | `#46695E` | boundary visible on light and dark surfaces |
| Primary text | `#E8F5EE` | text on dark surfaces |
| Muted text | `#9EB5AB` | secondary text on dark surfaces |

Use semantic system colors for native controls and preserve WCAG contrast for text. Teal and gold are accents, not body-text colors on white.

## Typography and voice

Use the platform system sans-serif for product UI and documentation. Use a system monospaced face for source IDs, hashes, formulas, units, and run records. Inquiry's voice is direct, careful, specific, and willing to abstain. Prefer “the source states” and “not verified in this run” over certainty theater.

## Accessibility, clear space, and minimum size

Keep clear space equal to the gold source-point diameter around the mark. Use the full mark at 48 px or larger and the supplied small optical mark at 16–32 px. Never remove the gold source point. The wordmark should not be used below 160 px wide.

On Ink (`#0A1F1A`), primary text (`#E8F5EE`) measures 15.302:1 contrast, Evidence teal measures 9.059:1, and Source gold measures 10.805:1. These exceed WCAG text or non-text contrast thresholds in their intended roles. Color is not the only cue: the cited point also differs by shape and position. Accessible product copy should say “cited point” or “selected source,” not only “the gold dot.” Every supplied SVG includes a title and description, but surrounding interfaces still need context-specific alternative text.

The final mark was mechanically rendered at 16, 32, 64, 128, 256, 512, and 1024 px and visually reviewed on light, dark, and monochrome surfaces. The 1024 px app-icon source is opaque so macOS can apply its own mask. Re-run `./script/check_brand.sh` and inspect the exact outputs after any geometry or color change.

## Do not

- stretch, rotate, skew, crop, add shadows, or rearrange the mark;
- recolor individual source lines or points outside the supplied one-color variants;
- place the full-color mark on a background that hides the frame boundary or cited point;
- use the mark to imply that BarnLabs verified, endorsed, certified, or medically approved third-party research;
- combine the mark with another organization's logo as if it were a joint product without written permission.

See [TRADEMARKS.md](TRADEMARKS.md) for name and logo use. Contact `hello@barnlabs.net` for an unlisted format.
