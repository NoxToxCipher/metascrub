# metascrub

Removes the information a file carries about you, and tells you exactly what it
found.

Photographs and documents hold a second payload that has nothing to do with what
they look like: GPS coordinates, camera serial numbers, the author's real name,
the editing-session identifiers that link two documents to one machine. Sending
a file sends all of it.

> **Status: working, not yet released.**
> Desktop application and command line tool, 253 tests, 20,000 fuzz cases with
> no panics. Linux, Windows and Android builds; packaged for Linux and
> ChromeOS. No signed builds yet.

## What makes this different

**It rebuilds files rather than editing them.** Deleting the metadata you
recognise leaves everything you do not: a vendor's private segment, a new chunk
type, a blob appended after the end-of-image marker. Each parser here walks the
input and copies across only what is on an explicit keep-list, so anything
unknown is dropped for not being on it. A 25 MB photograph from a Fujifilm
camera turned out to carry 39 kB of undocumented vendor data that a
denylist tool would have passed straight through.

**It says when it could not help.** A format it cannot take apart is reported as
such and returned untouched, and no output file is written. Claiming success on
a file nobody understood is the failure that actually harms someone.

**It separates what is provable from what is not.** Metadata removal is
verifiable: the bytes are in defined places and they are gone. Reducing a camera
sensor's fingerprint is statistical, so it lives in a different crate, behind a
setting that is off by default, reported in different words.

## What it handles

| Format | Approach | Assurance |
|---|---|---|
| JPEG, PNG, WebP, HEIF, AVIF | Container rebuilt from a keep-list | Complete |
| GIF | Block stream rebuilt from a keep-list; animation kept | Complete |
| TIFF | Each directory rebuilt from a keep-list; pixels copied verbatim; multi-page kept | Complete |
| SVG | Metadata, editor fields, scripts and external references removed | Best effort |
| XMP sidecar (.xmp) | Rebuilt as an empty packet; nothing it held survives | Complete |
| PDF | Object graph rebuilt | Best effort |
| Word, Excel, PowerPoint, OpenDocument | Archive parts cleaned, images inside recursed | Best effort |
| Camera raw (DNG, CR2, CR3, NEF, NRW, ARW, SR2, RW2, ORF, RAF, PEF, SRW, ERF, GPR, IIQ, MOS, 3FR, …) | Cleaned in place, never rebuilt. **Removed:** GPS, timestamps, owner/artist, XMP/IPTC, standard serial and image-ID fields, and the embedded preview's own EXIF. **Kept:** make/model and the vendor maker note. **Untouched:** the sensor data and decodability | Best effort |
| Sigma X3F raw | Recognised but returned untouched, reported as not cleaned | None |
| Video (MP4, MOV, MKV, WebM, AVI) | Recognised and named, but **not cleaned yet**; reported honestly with what videos leak (GPS, device, time) so it is never mistaken for clean | None |
| Audio (MP3, M4A, FLAC, OGG, WAV) | Recognised and named, not cleaned yet, reported honestly | None |
| Anything else | Returned untouched, reported as not cleaned | None |

A photo saved as a **Motion Photo / Live Photo** carries a whole short video
after its end marker. Cleaning the photo drops that video (it is trailing data),
and metascrub now reports it specifically rather than as anonymous bytes, so you
know the clip and its own location and time were there and are gone.

A camera raw is the sensor's near-unprocessed readout, not a picture — a
straight-from-the-camera JPEG is **not** a raw. Raws cannot be rebuilt from an
allowlist without corrupting the undocumented vendor sub-sections that hold the
actual sensor image (verified against real files from many brands), so they are
cleaned by careful in-place editing that never moves a byte. The **maker note is
deliberately kept**: manufacturers store the parameters a raw converter needs to
develop the file in the same block as the serial number, and removing it broke
real files. So a raw's internal serial number usually survives. To remove that
too — and to get a Complete clean — develop the raw into a JPEG or PNG first and
clean that. The desktop app's reference panel explains exactly what changes and
what does not, and how it differs by camera brand.

## Using it

**Desktop.** Drag files onto the window. It lists what it found in each one,
and writes cleaned copies only when you ask. Originals are never modified.

Drag and drop is the recommended path, and not only because it is quick. A file
picker is the thing that writes your filename into the desktop's recent-files
list, which is a record of exactly which files somebody thought were worth
cleaning. metascrub deletes those entries afterwards, but never creating one is
stronger than removing it.

**Command line.**

```bash
metascrub -n photo.jpg        # report only, write nothing
metascrub photo.jpg           # writes photo.clean.jpg
metascrub --json *.jpg        # machine-readable
```

Exit status `2` means a file was left uncleaned because its format is not
supported, so a script cannot mistake "not understood" for success.

## Installing on Linux

Download the archive for your machine, or the `.deb` on Debian, Ubuntu, Mint
and ChromeOS. `uname -m` says which architecture you have: `x86_64` is `amd64`,
`aarch64` is `arm64`.

```bash
tar xzf metascrub-0.1.0-linux-amd64.tar.gz
cd metascrub-0.1.0-linux-amd64
./install.sh              # into ~/.local, no root needed
sudo ./install.sh         # into /usr/local, for everyone
```

Or do not install anything. `./metascrub-gui` runs from wherever you unpacked
it, and `./metascrub` is the command line tool. Both are self-contained.

There is nothing to configure and nothing to keep up to date. metascrub writes
no configuration file, no cache and no state, so `./uninstall.sh` leaves
nothing behind.

**What it needs.** glibc 2.31 or newer, which covers every supported
distribution, and a graphical session for the window. No GTK, no Qt, no
toolkit to install: X11, Wayland and OpenGL are all reached at runtime, so one
binary works on GNOME, KDE, Xfce, a tiling window manager, and ChromeOS. The
command line tool is statically linked and needs nothing at all.

**ChromeOS.** Install the `.deb` into the Linux environment and it appears in
the launcher like any other app. See [`docs/chromeos.md`](docs/chromeos.md),
which also covers sharing folders with the Linux container.

## Building

```bash
cargo build --release
cargo test --workspace
```

No system dependencies. The toolchain is pinned in `rust-toolchain.toml`.

Building a *release* is a different matter, because a Linux binary demands the
glibc of whatever machine compiled it. See
[`packaging/linux/README.md`](packaging/linux/README.md).

## Crates

| Crate | What it is |
|---|---|
| `metascrub` | The library. No interface, no image decoding, no dependency on the others. |
| `pixelwash` | Sensor-fingerprint reduction. Separate because it decodes images. |
| `metascrub-gui` | Desktop interface. |

`metascrub` is usable on its own and is meant to be: it exists partly to be
embedded in other applications that need to clean a file before sending it.

## What this cannot do

- **It cannot make an upload anonymous.** Cleaning handles what is inside the
  file. If you upload while logged in, the platform has its own record of which
  account sent what and when.
- **It cannot remove a sensor fingerprint**, only reduce how well it matches.
- **It cannot help with a format it does not understand**, and it will say so
  rather than pretend.

The application explains all of this at length, including several widely
repeated pieces of advice that are wrong.

## Security

See [`SECURITY.md`](SECURITY.md) for how to report an issue, and for what is
explicitly not a vulnerability.

## Licence

GPL-3.0-or-later.
