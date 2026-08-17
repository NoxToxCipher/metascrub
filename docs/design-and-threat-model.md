# metascrub: Design and Threat Model

*Part of the Crake suite. This document states what metascrub does, how it does
it, what it deliberately does not do, and how to check every claim yourself. It
is written to be argued with. Where something cannot be promised, it says so.*

Status: beta (phase 1). Not yet externally audited. Video and audio are
recognised but not yet cleaned.

---

## 1. What metascrub is

metascrub removes the information a file carries about the person who made it:
the GPS coordinates in a photo, the camera serial number, the timestamps, the
author name in a document, the editing history, the embedded thumbnail that
still shows the uncropped original.

It runs entirely on your own machine. It has no network capability of any kind.
It never modifies your original file; it writes a cleaned copy.

It is one tool. It cleans the *file*. It does not, and cannot, make you
anonymous. Section 3 is the boundary of what it protects, and it matters more
than the rest of this document.

## 2. The one claim

Everything metascrub does is in service of a single claim: **what it tells you
about a file is true.** So the interface is built around an honesty grade, not a
progress bar, and every result carries one of three assurance levels:

- **Complete.** The container was rebuilt from scratch, keeping only an explicit
  list of the parts worth keeping. Nothing outside that list survives, including
  structures the tool has never seen. This is the strongest guarantee.
- **Best effort.** Known metadata was removed, but the file was edited rather
  than rebuilt, so an unrecognised private field could remain. The report says
  what was not guaranteed.
- **Not cleaned.** The format could not be taken apart. No output is written,
  because a file named "clean" that was never cleaned is worse than no file.

## 3. Threat model: what it protects, and what it does not

metascrub defends against **the information inside a file** being used to
identify or locate the person who made or sent it. That is the whole of its job.

It does **not** defend against, and no file-cleaning tool can:

- **Your connection.** Cleaning a file does not hide your IP address or that you
  uploaded it. That is Tor's job, not metascrub's.
- **Your account.** If you upload while logged in, the platform knows which
  account did so, when, and from where. That record was never in the file.
- **The platform's own records and markers.** Some services write their own
  identifiers into images (for example, an IPTC field on upload). metascrub
  removes those on a file it is given, but it cannot remove the copy the platform
  kept.
- **The original still on your disk.** metascrub writes a clean copy and never
  touches the original, which still contains everything. You must delete the
  original yourself, and secure deletion is itself unreliable on modern drives.
- **The camera's sensor fingerprint** (PRNU), which lives in the pixels, not the
  metadata. A separate, optional, weaker tool addresses that; see section 7.
- **Perceptual hashing.** Platforms match images by content in a way that
  survives resizing and re-compression. metascrub does not change what a picture
  *is*.

The most dangerous idea this tool could leave you with is that a clean file is
an anonymous one. It is not. It is a clean file.

## 4. How it cleans: the allowlist rebuild

For every format it fully supports, metascrub does not search for known-bad
metadata and delete it. It does the opposite: it **rebuilds the file from a list
of the structures worth keeping**, and copies only those into a fresh file.

This matters because a delete-what-you-recognise approach silently passes through
anything new, private, or deliberately hidden. Rebuilding from a keep-list makes
the default for anything unrecognised "drop it". This is what earns the
*Complete* grade.

The pixel or document data is copied through unchanged. metascrub does not
re-encode images or recompress them, so there is no quality loss; only the
container is new.

Known incident classes this design is immune to by construction: trailing-data
after the end marker (the aCropalypse class, where a cropped image still carried
the uncropped original), the PDF incremental-update leak (where "redacted" text
survived under the black boxes), and pass-through of vendor-private blocks.

## 5. Camera raw files: why they are different

A camera raw (CR2, CR3, NEF, ARW, DNG, RW2, ORF, RAF, and others) cannot be
rebuilt from a keep-list. Its real sensor image lives in vendor-specific
sub-sections whose layout is undocumented and different for every manufacturer.
Rebuilding one would hand back a corrupted file that no longer opens, and a raw
cannot be reshot. Testing against real files from many brands confirmed this.

So raws are cleaned **in place**: metascrub walks the file, overwrites exactly
the structures known to identify a person, a place, or one specific camera, and
moves nothing else. The sensor data is never touched and the file length does
not change. This is always *Best effort*.

Two deliberate choices:

- The camera **make and model are kept**. A raw converter needs them to decode
  the file, and they identify a model owned by many people, not you.
- The manufacturer's **maker note is kept**, because it also holds the settings a
  converter needs to develop the raw, and removing it corrupted real files. The
  maker note usually contains the camera's internal serial number, so on a raw
  that serial typically survives. metascrub says so, on every raw, in a "still in
  the file" disclosure that names what remains and what it would reveal. To
  remove the serial too, develop the raw to a JPEG and clean that.

Corrupting an irreplaceable file is the one outcome metascrub refuses. A raw it
cannot safely take apart (Sigma X3F) is returned untouched and reported as not
cleaned, never cleaned badly.

## 6. Verifying its own work

Two properties let you check metascrub rather than trust it:

- **Verify-after-clean.** On request, metascrub re-scans its own output to
  confirm nothing it removes survived, and cleans the same input twice to confirm
  the result is byte-identical. A *Complete* clean that let metadata slip
  through fails this check loudly.
- **Deterministic output.** The same input and settings always produce the same
  bytes, on any machine, every run. This proves nothing that varies per run (a
  stray timestamp, random padding) can leak into the file, and it lets anyone
  reproduce a clean and compare.

## 7. Sensor fingerprint reduction (pixelwash)

A camera leaves a faint fixed pattern (Photo Response Non-Uniformity) in the
pixels of every photo it takes. It is not metadata; removing EXIF does nothing to
it. It can be used to show that two photographs came from the same sensor.

metascrub includes an optional, off-by-default tool that denoises, downscales,
adds a little noise, and re-encodes, which reduces how strongly the pattern can
be matched. This is a **statistical, best-effort** claim, deliberately kept
separate from the metadata guarantee and reported in different words. It reduces
correlation; it does not remove the fingerprint, and nothing in the tool will
ever say it does. It also only applies to a camera-captured raster photo: it
cannot apply to a raw (that would destroy the raw) or to a non-photo file.

## 7a. Serving an embedding client: the render path (built)

The messenger in this suite needs something the always-on metadata path does not
offer. It receives pictures from people who are not using our software, has to
draw them, and must never hand a stranger's bytes to the browser engine that
draws its interface. Its design is recorded in that project's §79; this section
is the half that lives here.

**The job.** Take an arbitrary inbound image, produce a PNG that is safe to
display and carries none of the source's metadata, and leave the stored original
alone. Concretely: decode, optionally downscale to a display size, re-encode as
PNG, return that. Refuse anything whose declared dimensions are absurd *before*
allocating for them.

**What is built.** `pixelwash::to_png` does exactly this: it reuses the same
bounded decoder as the wash path (allowlist, pre-decode dimension check against a
megapixel cap, decode, zero-dimension guard), downscales with Lanczos3 so the
longest edge is at most `max_edge` (never enlarging a smaller image), and
re-encodes as PNG from the raw pixels — dropping every scrap of source metadata
and preserving any alpha channel. It is exposed to native hosts as `ms_to_png` in
`metascrub-ffi`, which additionally runs the PNG back through the sanitizer so the
render path makes the same honest allowlist guarantee as every other output. The
conversion is deliberately *not* wired into `metascrub`; see the next paragraph.

**Where it goes, and why it is not negotiable.** In `pixelwash`, never in
`metascrub`. Section 8 makes a specific promise — "the always-on metadata path
never decodes a pixel", and the image decoder "is reached only through the
fingerprint tool, which is off by default". That is why running metascrub on a
hostile file is safe, and it is a claim we make in public. A converter decodes by
definition. Putting one in `metascrub` would silently retire the promise, so the
crate split that already exists is load-bearing and stays: `metascrub` walks
containers with no image dependency, `pixelwash` decodes and is opt-in.

**What the render path does and does not buy.** It removes the *embedding
application's* exposure to a decoder it does not control, by making sure the only
thing that application's renderer ever parses is a PNG we wrote. It does nothing
for the decode itself, which is still a decode of hostile input — that risk is
contained by the caller running this in a sandbox, not by anything in this
crate. This document should never imply otherwise.

**Conversion is not fingerprint reduction.** Changing a file from JPEG to PNG
preserves the decoded pixels exactly, and PRNU lives in the pixels. Repeated
conversion does not help: each lossy round degrades correlation slightly and
unpredictably, and destroys the picture long before it destroys the pattern.
Only the geometry changes in section 7 — downscale, denoise, added noise — do
real work. A caller who downscales for display gets some of that as a side
effect and must not be told it is pixelwash.

## 8. Security properties

- **No network capability.** metascrub makes zero network connections. There are
  no sockets, no HTTP client, nothing. A continuous-integration job runs the
  suite with networking removed to prove it. Verifiable, and the strongest
  privacy property the tool has.
- **No `unsafe` code** (`#![forbid(unsafe_code)]`), so the tool's own logic
  cannot corrupt memory. Parsing is done with a bounds-checked reader that
  returns an error rather than indexing out of range.
- **A parser crash is a clean stop.** The build aborts on panic, so a malformed
  file cannot leave the program in an exploitable state. The parsers are fuzzed
  against random, mutated, and truncated input.
- **Denial-of-service hardening.** Recursion is depth-limited, decompression is
  bounded (a compression bomb is refused before it is expanded), array lengths
  and offsets are capped, and every loop over untrusted structure is bounded.
- **The pixel decoder is opt-in.** The image-decoding library, historically the
  most exploit-prone dependency, is reached only through the fingerprint tool,
  which is off by default. The always-on metadata path never decodes a pixel.
- **Local traces.** On Windows the tool clears the Recent-Items shortcuts a file
  dialog creates, and it suppresses crash dumps so a crash cannot write a photo
  or its coordinates to disk. Sensitive buffers are zeroized.

### Residual risks (stated plainly)

- The PDF parser is a third-party library handling a very complex format and is
  the largest always-on parser of untrusted input. It should be fuzzed
  specifically, and dependency advisories are checked in CI.
- Memory hygiene is partial: intermediate buffers are not all zeroized, and on a
  crash the zeroizing buffers do not run. Freed pages can linger. This matters
  on a compromised or forensically examined machine.
- The original file persists on disk, and the tool cannot securely delete it.
- While processing, data can be paged to unencrypted swap. Use full-disk
  encryption for high-threat work.

## 9. Format coverage

| Format | Approach | Assurance |
|---|---|---|
| JPEG, PNG, WebP, HEIF, AVIF, GIF, TIFF | Rebuilt from a keep-list | Complete |
| SVG | Metadata, scripts and external references removed | Best effort |
| XMP sidecar | Rebuilt as an empty packet | Complete |
| PDF | Object graph rebuilt | Best effort |
| Word, Excel, PowerPoint, OpenDocument | Archive parts cleaned, embedded images cleaned | Best effort |
| Camera raw (CR2, CR3, NEF, ARW, DNG, RW2, ORF, RAF, and more) | Cleaned in place; sensor data untouched; maker note kept and disclosed | Best effort |
| Video, audio | Recognised and named; not cleaned yet; the report says so | Not cleaned |
| Sigma X3F, anything else | Returned untouched, reported as not cleaned | Not cleaned |

A photo saved as a Motion Photo or Live Photo carries a whole short video after
its end marker; metascrub drops that video and reports it specifically.

## 10. How to check these claims yourself

- **Cross-check with an independent tool.** Run the cleaned file through ExifTool
  (a separate codebase). metascrub was validated this way against 1,343 real
  photographs and a corpus of real camera raws from every major manufacturer.
- **Use the verify flag.** It re-scans the output and confirms reproducibility.
- **Read the source, and reproduce the build.** The source is published. Signed
  releases and reproducible builds let you confirm the binary matches the source.
- **Confirm it never connects.** Run it with networking disabled; it behaves
  identically.

## 11. What is not done yet

- Video and audio cleaning.
- An external security audit.
- Reproducible builds and signed releases (in progress).
- Per-format raw testing beyond a few real samples of each vendor.
- **A sealed report format.** `report.rs` records what was removed and under
  which `Kind`. An embedding application wants to keep that beside a file it has
  cleaned, so a person who needs to know when a photograph was taken can be told
  without the file carrying the answer. Nothing serialises a report for storage
  today.

These are stated here rather than left for a user to discover, because a tool
that hides its limits is more dangerous than one that names them.
