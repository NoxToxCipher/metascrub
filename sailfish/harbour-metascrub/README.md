# metascrub for Sailfish OS

A **native Silica app** over the pure-Rust metascrub core. Same crown jewels as
the CLI and the Android app — the audited scrubbing and PRNU logic — with a Qt
Creator / Silica interface that belongs on the OS instead of being ported onto
it.

## Architecture

```
qml/  (Silica UI)  ──►  src/scrubber.cpp  ──►  crates/metascrub-ffi  ──►  metascrub + pixelwash
  ScrubPage             Scrubber QObject       C ABI (metascrub.h)        (the Rust core, unchanged)
  HandbookPage          QJson ⇄ QVariantMap
  CoverPage
```

- **`crates/metascrub-ffi`** (in the workspace, not here): a plain **C ABI** over
  the core — `ms_sanitize`, `ms_report_json`, `ms_reduce_fingerprint`, plus the
  two free functions. It is the second FFI crate in the workspace (the first is
  the Android JNI one); both are the audited `unsafe` exceptions to the workspace
  `#![forbid(unsafe_code)]` rule. It compiles on the host today (`cargo build -p
  metascrub-ffi`).
- **`src/scrubber.cpp`**: a `QObject` that reads a file, calls the C ABI, and
  parses the report JSON into a `QVariantMap` for QML — the same
  assurance/removed/warnings contract every metascrub front end uses. It carries
  over the Android app's save-time re-inspect guard, so a file the core cannot
  clean is never written out as a "cleaned copy".
- **`qml/`**: the Silica interface. Every assurance badge pairs colour with a
  word (COMPLETE / BEST EFFORT / NOT CLEANED), never colour alone. `handbook.json`
  is copied from the Android tree so the words live in one place.

## What is wired vs what needs the SDK

**Done and self-consistent:** the C ABI (builds on host), the `Scrubber` backend,
`main.cpp`, all three QML pages, the cover, the `.pro`, the RPM spec, the
`.desktop` (with a Sailjail permission set that **omits Internet** — the same
verifiable no-network stance as the zero-permission Android build).

**Left for when the SDK/emulator is up:**
1. **The Rust cross-compile** — the one real integration task (see below).
2. **File-picker wiring** — `ScrubPage` opens `Sailfish.Pickers`; confirm the
   exact `selectedContentProperties` signal against the installed Pickers version.
3. **Save-destination picker** — the `scrubber.save(...)` call is ready; only the
   `FolderPickerPage` that supplies `destPath` is stubbed (marked `TODO(SDK)`).
4. **Icon assets** — drop `harbour-metascrub.png` in the hicolor sizes
   (86/108/128/172) from the existing sandpiper mark.
5. **Translations** — the QML uses `qsTr(...)`; the 11 languages already written
   for Android can be brought over as Qt `.ts` files.

## Building

Prerequisites: the Sailfish SDK + emulator (installers already staged in
`~/Downloads`), and a Rust toolchain with the Sailfish targets:

```bash
rustup target add armv7-unknown-linux-gnueabihf \
                  aarch64-unknown-linux-gnu \
                  i686-unknown-linux-gnu     # emulator (i486)
```

The RPM spec's `%build` maps the RPM `%{_target_cpu}` to the Rust triple, builds
`metascrub-ffi` for it, then links it into the Silica app:

```bash
# from the repo root, inside the Sailfish build engine (sfdk / mb2):
sfdk config target=SailfishOS-4.x-i486        # or the arm targets for a device
sfdk build sailfish/harbour-metascrub
```

### The hard part: Rust in the Sailfish build engine

The core is Rust; the Sailfish build engine is where the cross-compile has to
succeed. Two routes, both proven by other Sailfish+Rust apps:

- **Inside the build engine:** install `rust`/`cargo` in the target and let the
  spec's `cargo build --target <triple>` run there (what the spec assumes).
- **Cross from the host:** build the staticlib with `rustup` + the Sailfish
  sysroot as the linker, then point `RUST_LIB_DIR` at the output.

This is the integration step to nail first once the emulator is up — everything
above it (ABI, backend, UI) is already in place and the ABI already builds.
