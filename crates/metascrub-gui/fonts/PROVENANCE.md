# Bundled font provenance

`Padauk-Regular.ttf` is embedded into `metascrub-gui` (via `include_bytes!`) so
the Burmese (Myanmar) interface renders. egui's default fonts cover Latin and
Cyrillic but not the Myanmar script, so without this the entire Burmese UI shows
empty boxes.

| Field | Value |
|---|---|
| Font | Padauk (Regular) |
| Designer / owner | SIL International |
| Licence | SIL Open Font License 1.1 (`OFL-Padauk.txt`, alongside) |
| Source | Google Fonts repository, `ofl/padauk/Padauk-Regular.ttf` |
| Source URL | https://raw.githubusercontent.com/google/fonts/main/ofl/padauk/Padauk-Regular.ttf |
| Downloaded | 2026-08-13 |
| Size | 498860 bytes |
| SHA-256 | `c89cf56e572abda9652d9e54203bd729b0c59541c4b569046b9b61acd0b532f3` |

## Why this is pinned

The font is a third-party binary blob that `cargo`/`cargo deny` cannot see (it
is not a crate), and it is parsed at runtime by the text stack. A swapped or
tampered file would otherwise be embedded silently. The size is checked at
compile time in `main.rs` (`install_fonts`), and the SHA-256 above is the
authoritative integrity value.

## To re-verify

```bash
sha256sum crates/metascrub-gui/fonts/Padauk-Regular.ttf
# must print: c89cf56e572abda9652d9e54203bd729b0c59541c4b569046b9b61acd0b532f3
```

If you update the font, re-download from the source URL, update the size and
SHA-256 here and the length constant in `main.rs`, and record the new download
date. Keep the OFL licence file current alongside it.
