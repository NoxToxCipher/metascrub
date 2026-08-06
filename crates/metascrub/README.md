# metascrub

Format-aware metadata removal for images and documents.

A photograph carries a second payload that has nothing to do with what it looks
like: where it was taken, which camera took it, that camera's serial number, and
a thumbnail from before you cropped it. A document carries who wrote it, who
edited it, how long they spent, and tokens that link it to every other document
edited on the same machine. Sending the file sends all of it.

`metascrub` removes that payload. It is a plain Rust library with no dependency
on the rest of this workspace, because it has two jobs: sanitizing outgoing file
transfers in the Tox client ([`DESIGN.md` §9.1](../../DESIGN.md)), and being the
engine of a standalone metadata remover later.

> **This is not the PRNU protection.** Stripping metadata does not touch pixels,
> so it does nothing about the sensor fingerprint carried in the pixel values
> themselves. That is a separate, lossy protection (DESIGN §9.2) and it is
> deliberately not in this crate, so that nothing here can imply it happened.

## Design: allowlist, never denylist

The obvious approach is to find the known metadata blocks and delete them. That
fails silently on anything it was not taught about: a vendor's private JPEG
segment, a PNG chunk type invented next year, a blob appended after the
end-of-image marker.

So the container is not edited, it is **rebuilt**. Each parser walks the input,
copies across only the structures on an explicit keep-list, and drops everything
else *including structures it does not recognise*. New, private and deliberately
hidden data is discarded by default, because it was never on the list.

Where a format makes that impossible, the crate says so rather than pretending.
Every result carries an assurance level:

| Assurance | Meaning |
|---|---|
| `Complete` | Rebuilt from a keep-list. Nothing outside the list survived. |
| `BestEffort` | Known metadata was removed or overwritten, but the container was not rebuilt, so an unrecognised structure could remain. The warnings say what. |
| `None` | Nothing was removed. The file is returned exactly as it arrived. |

An unsupported format is `None`, not an error, and never reports "clean".

## Re-encode versus strip

The rebuild is byte-exact on the compressed image data, so there is no
generational quality loss. Re-encoding would also guarantee removal, but it
costs quality on every pass and is no safer for metadata, because an allowlist
rebuild already carries nothing but pixels forward. The tests decode the input
and the output and assert the pixels are identical.

## What is removed, per format

### JPEG — `Complete`

| Removed | Kept |
|---|---|
| EXIF (`APP1`), including GPS, timestamps, camera and lens identity | Quantization and Huffman tables, frame and scan headers |
| Maker notes (serial numbers, shutter counts) | The entropy-coded image data, byte for byte |
| XMP and Extended XMP (`APP1`) | JFIF pixel density, rebuilt canonically |
| IPTC and Photoshop image resource blocks (`APP13`) | The Adobe colour transform (`APP14`), rebuilt canonically |
| EXIF thumbnail (IFD1), JFIF thumbnail, JFXX, MPF preview index | ICC profile, with `--keep-icc` |
| Comments (`COM`) | EXIF orientation only, with `--keep-rotation` |
| **Every other `APPn`**, known or not | |
| **Everything after the end-of-image marker** | |

The Adobe segment is kept because dropping it makes a CMYK or YCCK JPEG decode
with inverted colours, and it carries no identifying content. It is rebuilt from
scratch with its flag words zeroed rather than copied.

### PNG — `Complete`

| Removed | Kept |
|---|---|
| `tEXt`, `zTXt`, `iTXt` (`iTXt` is where XMP lives) | `IHDR`, `PLTE`, `IDAT`, `IEND` |
| `eXIf`, including GPS | `tRNS`, `gAMA`, `cHRM`, `sRGB`, `sBIT`, `bKGD`, `pHYs`, `hIST` |
| `tIME`, `dSIG` | `acTL`, `fcTL`, `fdAT` (APNG animation) |
| **Every other chunk type**, including private ones | `iCCP`, with `--keep-icc` |
| **Everything after `IEND`** | |

CRCs are verified on chunks that are kept. A bad CRC there means the image data
is damaged and is reported as an error rather than copied through.

### WebP — `Complete`

Removes the `EXIF` and `XMP ` chunks, every unrecognised chunk, and anything past
the declared RIFF length. Keeps `VP8 `, `VP8L`, `VP8X`, `ALPH`, `ANIM` and `ANMF`,
and `ICCP` with `--keep-icc`. The `VP8X` feature flags are cleared to match, so
the header stops advertising chunks that are no longer there.

### HEIF / HEIC / AVIF — `BestEffort`

Removes the `Exif` and XMP items from the `meta` box, and the payload of any
top-level XMP `uuid` box.

**How, and why it matters:** HEIF stores metadata in the `mdat` blob and
addresses it by absolute file offset. Cutting those bytes out would shift every
other offset in the file, so a real excision means rewriting the item and
location tables, and a mistake there produces a file that still opens while
quietly keeping some of what we claimed to remove. Instead the metadata's byte
range is **overwritten in place** with an empty-but-valid EXIF block or XMP
packet. The values are gone; the empty item entries that held them remain, so a
tool that asks "does this file have EXIF?" will still say yes. Unknown boxes are
carried through rather than dropped.

Metadata stored with construction method 2 (inside another item) is declined
loudly rather than guessed at.

### PDF — `BestEffort`

| Removed | Kept |
|---|---|
| The document information dictionary: Author, Title, Subject, Keywords, Creator, Producer, creation and modification dates | Page content, fonts, images |
| XMP metadata streams, and the references to them | Annotation body text |
| `/PieceInfo` and `/LastModified` (private application state) | Form fields, JavaScript, optional content |
| Annotation author (`/T`), dates (`/M`, `/CreationDate`), and `/NM` | |
| The trailer file identifier `/ID` | |
| **Superseded revisions left by incremental updates** | |

This is the one format parsed with a library (`lopdf`) rather than by hand,
because objects are addressed by byte offset and modern producers pack the
information dictionary inside a compressed object stream that a byte-level
editor cannot reach. Rebuilding from the parsed object graph also drops any
earlier revisions the file was carrying, which an append-only edit would leave
sitting in the file.

**Not reached:** attached files keep their own metadata, and images inside page
content are not descended into. Both are reported when present. Encrypted PDFs
are refused rather than reported clean.

### OOXML (.docx, .xlsx, .pptx) and OpenDocument (.odt, .ods, .odp) — `BestEffort`

| Removed | Kept |
|---|---|
| `docProps/core.xml`: creator, last modified by, revision, dates | Document content |
| `docProps/app.xml`: application, version, company, manager, template, total editing time | Styles, numbering, themes, fonts |
| `docProps/custom.xml`: all custom properties | Charts, embedded objects (flagged) |
| OpenDocument `meta.xml`: creator, editing cycles, generator, dates | |
| Tracked-change and comment authors, initials and dates, in every part | |
| Revision-save identifiers (`w:rsid*`) and the `w:rsids` block | |
| EXIF in embedded images (`word/media/`, `xl/media/`, `ppt/media/`, `Pictures/`) | |
| ZIP entry timestamps, extra fields (Unix uid/gid, NTFS times), comments | |
| The archive's host-system byte | |

The property parts are replaced wholesale with a canonical empty version, not
edited, so nothing in them can survive in a field we did not think to clear.
They are emptied rather than deleted because deleting them leaves a dangling
relationship that Word reports as a corrupt file.

Unknown parts are **kept**, which is the opposite of the image formats. In these
formats an unrecognised part is as likely to be a chart, a font or an embedded
object the document needs, so dropping it would routinely break files. Macros,
`customXml` data and embedded objects are kept and flagged in the report.

ZIP64 archives and encrypted archives are declined rather than misparsed.

### Anything else — `None`

Returned byte for byte, with a warning saying it was not sanitized. Reporting
"clean" on a file we did not understand is the one failure mode that actively
harms the user.

## What this does not do

- **Filenames.** Names leak, and neutralizing them is the caller's decision
  since it depends on where the file is going.
- **Pixel-level fingerprints.** See the note at the top.
- **Video.** Not yet.
- **Content.** A photo of your street is still a photo of your street. Removing
  a GPS tag does not make an image anonymous.

## Using it

```rust
use metascrub::{sanitize, Policy};

let photo = std::fs::read("holiday.jpg")?;
let clean = sanitize(&photo, &Policy::default())?;

if clean.report.found_location {
    eprintln!("this photo recorded where it was taken");
}
for item in &clean.report.removed {
    eprintln!("  removed {} at {}", item.kind, item.location);
}
std::fs::write("holiday.clean.jpg", &clean.data)?;
```

`Policy::default()` is the strict configuration. Two settings relax it:

- `orientation: PreserveMinimal` rebuilds a fresh EXIF block holding the
  orientation tag and nothing else, so photos do not display sideways. The tag
  is one of eight values and identifies nobody, but the file then still has an
  EXIF marker in it.
- `color_profile: Keep` retains ICC profiles. They are dropped by default
  because a profile is a variable-length blob we do not parse, carrying a
  free-text description and a manufacturer and model pair.

`inspect()` runs the same parsers and returns only the report, for a preview
before the user commits to sending.

## Command line

```bash
cargo run -p metascrub -- --dry-run photo.jpg
```

```
photo.jpg: JPEG: removed 6 item(s), 8214 bytes, including GPS coordinates (complete assurance)
  ** this file recorded where it was taken **
  - EXIF at APP1, 8104 bytes
  - maker note at APP1 EXIF maker note
  - thumbnail at APP1 EXIF IFD1
  - XMP at APP1, 74 bytes
  - comment at COM, 22 bytes
  - trailing data at after EOI, 14 bytes
```

By default the cleaned copy is written alongside the original as
`photo.clean.jpg`, so nothing is overwritten until asked. `--in-place`, `-o` and
`--suffix` change that; `--json` gives machine-readable output. Exit status is
`0` on success, `1` on a failure, and `2` when a file was left unsanitized
because its format is unsupported.

## Features

| Feature | Adds | Dependency |
|---|---|---|
| `image` | JPEG, PNG, WebP, HEIF, AVIF | none |
| `ooxml` | OOXML, OpenDocument | `miniz_oxide` |
| `pdf` | PDF | `lopdf` |
| `cli` | the `metascrub` binary | none |

All are on by default. `--no-default-features --features image` is a build with
no dependencies at all beyond `thiserror`, which suits an Android or embedded
target that only handles photographs.

## Testing

```bash
cargo test -p metascrub
```

The unit tests work on hand-built containers to exercise the parsing edge cases.
The integration tests encode real images with a real encoder, plant metadata in
them, sanitize, then **decode the result and compare pixels**, which is the
property that actually matters. Every parser is also fed every truncation and a
few thousand single-byte corruptions of its own fixtures, because a panic in a
crate that opens files from strangers is a denial of service.

## Licence

GPL-3.0-or-later, matching the workspace.
