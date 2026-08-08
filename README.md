# metascrub

Removes the information a file carries about you, and tells you exactly what it
found.

Photographs and documents hold a second payload that has nothing to do with what
they look like: GPS coordinates, camera serial numbers, the author's real name,
the editing-session identifiers that link two documents to one machine. Sending
a file sends all of it.

> **Status: working, not yet released.**
> Desktop application and command line tool, 173 tests, 20,000 fuzz cases with
> no panics. No installer or icon yet, and no signed builds.

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
| PDF | Object graph rebuilt | Best effort |
| Word, Excel, PowerPoint, OpenDocument | Archive parts cleaned, images inside recursed | Best effort |
| Anything else | Returned untouched, reported as not cleaned | None |

## Using it

**Desktop.** Drag files onto the window. It lists what it found in each one,
and writes cleaned copies only when you ask. Originals are never modified.

**Command line.**

```bash
metascrub -n photo.jpg        # report only, write nothing
metascrub photo.jpg           # writes photo.clean.jpg
metascrub --json *.jpg        # machine-readable
```

Exit status `2` means a file was left uncleaned because its format is not
supported, so a script cannot mistake "not understood" for success.

## Building

```bash
cargo build --release
cargo test --workspace
```

No system dependencies. The toolchain is pinned in `rust-toolchain.toml`.

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
