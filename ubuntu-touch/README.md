# metascrub for Ubuntu Touch

A native Lomiri app over the same pure-Rust core the desktop, Android and
Sailfish builds use. Hand it a photo or a document, see honestly what it carries,
and get a cleaned copy back — all on the phone.

> **Status: runs. A photo with GPS in it has been scrubbed end to end through
> this interface, on a desktop, against the real Lomiri toolkit. Never packaged
> by clickable, never started on a phone, and no Content Hub transfer has ever
> completed.** The section
> [What is actually verified](#what-is-actually-verified) says exactly which
> parts are proven and which are still claims.

## Why this platform is a good fit

Ubuntu Touch confines apps by default, and metascrub is a tool whose whole
argument is that it wants almost nothing. Those line up better here than
anywhere else the app has been ported to.

- **The no-network claim stops being a promise.** The app's AppArmor profile
  asks for two things: to receive files from other apps, and to give files back.
  It does not include the `networking` policy group, so the system refuses the
  app a socket. The profile is four lines
  ([`metascrub.apparmor`](metascrub/metascrub.apparmor)) and anyone can read it
  inside the installed package. CI fails the build if `networking` ever appears
  there.
- **It cannot read your files, and that is the design.** A confined app sees its
  own two directories and nothing else. Files arrive only through the Content
  Hub, when a person deliberately hands one over. There is no folder picker
  here because there cannot be one, and that is the better behaviour.
- **The one flow that matters is native to the platform.** A messenger asking
  for a photo can ask *metascrub* for it. The app cleans the photo and hands
  back the clean copy, so the original never reaches the messenger at all.

## The three ways in

| How it starts | What happens |
|---|---|
| From the launcher | The user picks files through the Content Hub, scrubs, then saves cleaned copies wherever they choose |
| Another app shares files **to** metascrub | They arrive already copied into this app's storage. The screen says plainly that nothing was sent anywhere, because "share" is exactly the word that makes people think it was |
| Another app asks metascrub **for** a file | metascrub cleans first and charges the transfer with the cleaned copy instead of the original |

## What is in here

```
ubuntu-touch/
  build-ffi.sh                  builds the Rust core for arm64 / armhf / amd64
  metascrub/
    clickable.yaml              the click build
    CMakeLists.txt              links the core into one binary, lays out the package
    manifest.json.in            click metadata (name, framework, hooks)
    metascrub.apparmor          the confinement profile — no networking
    metascrub-contenthub.json   pictures and documents, in and out
    metascrub.desktop.in        launcher entry
    src/main.cpp                registers the backends, loads the QML
    src/workspace.{h,cpp}       the app's own storage, and clearing it
    qml/                        Lomiri interface
    tests/scrubber_smoke.cpp    the backend, end to end, through the real core
```

The Qt backend itself is not here: `native/scrubber.cpp` at the repository root
is shared with the Sailfish app, so the rule that a file which cannot be taken
apart is never written out as "cleaned" exists in one place. See
[`native/README.md`](../native/README.md).

The Handbook text is not here either. `CMakeLists.txt` installs
`android/app/src/main/res/raw/handbook.json` directly, so there is no second copy
to drift.

## Building

Two steps, because the Rust core has to exist before the app links it.

```bash
# 1. the core, for the architecture you are building
ubuntu-touch/build-ffi.sh arm64        # or armhf, or amd64
                                       # inside clickable: clickable script ffi

# 2. the click
cd ubuntu-touch/metascrub
clickable build --arch arm64
clickable install --arch arm64         # to a device over adb/ssh
```

Prerequisites for step 1 when cross-compiling on a host:

```bash
rustup target add aarch64-unknown-linux-gnu       # arm64
rustup target add armv7-unknown-linux-gnueabihf   # armhf
sudo apt install gcc-aarch64-linux-gnu gcc-arm-linux-gnueabihf
```

The core is linked in as a static library, so the click ships one binary and no
loose `.so` to find at runtime. If cmake cannot find the library it stops and
tells you which command to run, rather than building an app with no core in it.

### Running it on a desktop, without a phone

Lomiri is packaged in the Ubuntu archive, so the interface runs on an ordinary
desktop. This is how the app was first debugged, and it is the cheapest way to
see a change.

```bash
sudo apt install qtbase5-dev qtdeclarative5-dev \
                 qml-module-lomiri-components qml-module-lomiri-content

cd ubuntu-touch/metascrub
cmake -S . -B build -DMETASCRUB_TESTS=ON -DCMAKE_INSTALL_PREFIX=$PWD/build/install
cmake --build build && ./build/scrubber_smoke
cmake --install build && ./build/install/metascrub
```

Headless, for a screenshot:

```bash
Xvfb :99 -screen 0 480x800x24 &
DISPLAY=:99 ./build/install/metascrub &
DISPLAY=:99 import -window root shot.png
```

## What is actually verified

On amd64, Qt 5.15, against the Lomiri UI toolkit 1.3.5100 and the
lomiri-content-hub QML module 1.1.1 from the Ubuntu 24.04 archive:

- The Rust core cross-compiles for `aarch64`, `armv7hf` and `x86_64`, and
  `build-ffi.sh` puts each one where the build looks for it.
- The app and the test compile and link against the real core.
- `scrubber_smoke` passes fifteen checks against the real library: a tagged PNG
  is reported `complete`, its text chunks are found, the cleaned copy has nothing
  left to remove and retains nothing, an unknown format is never reported as
  cleanable and **writes no file**, and a missing file fails with a message.
- **The interface runs**, with no warnings the toolkit does not also emit about
  itself. The scrub page, the Handbook and About all render, and the page stack
  and back actions work.
- **A whole scrub runs through the interface.** A JPEG carrying GPS
  coordinates, make, model, artist and a capture time, plus a PNG carrying a
  comment, plus an unreadable `.dat`, were queued and scrubbed. The two photos
  came back `COMPLETE`, the JPEG showed "Recorded where it was taken", and the
  `.dat` came back `NOT CLEANED` with nothing written for it. ExifTool, a
  separate codebase, finds none of those fields in the saved copies, which were
  written under random names.
- `cmake --install` lays out a click tree with the binary, the QML, the icon and
  all four metadata files in the right places, and the app runs from that tree.
- Every QML file parses (`qmllint`), and every JSON file in the package is valid.

Still not verified:

- **The package has never been built by `clickable` or installed on a device.**
  No emulator, no phone, no `clickable` in this environment.
- **Desktop Lomiri is not the phone.** These modules come from Ubuntu 24.04,
  while the click targets the focal image. Versions differ; layout on a real
  screen, at a real grid unit, with a real on-screen keyboard, is untested.
- **No Content Hub transfer has ever completed.** The picker instantiates and
  correctly reports that no peer app is installed here, but choosing a peer,
  building `ContentItem`s and charging a transfer are all unexercised, in both
  directions. Import, share-in and export-back remain the likeliest place for a
  first-run bug on a device.
- **The framework and policy version** (`ubuntu-sdk-20.04`, `20.04`) target
  focal. A 24.04 image will want both bumped, in `CMakeLists.txt` and
  `metascrub.apparmor`.

The queue was seeded for that run by a temporary local patch, since there is no
Content Hub peer here to hand files over. The patch was not committed: the
shipped app has no way to be handed a file except through the Content Hub.

## Known gaps

- **No translations.** The interface uses `i18n.tr` throughout, so the strings
  are ready to extract, but there is no `po/` yet. Android already ships eleven
  languages and the Handbook is translated with it; wiring those up here is the
  obvious next job.
- **No OpenStore submission.** The maintainer address in `manifest.json.in` is a
  GitHub noreply placeholder, and the app id namespace (`metascrub.noxtoxcipher`)
  has to match whatever OpenStore account publishes it.
- **Clearing working files is an ordinary delete.** On flash storage the blocks
  may survive until they are reused. The About screen says so.
- **Everything runs on the interface thread.** Inspecting is cheap — the core
  walks containers and never decodes an image — but the optional camera
  fingerprint wash does decode, denoise and re-encode, so a thorough wash of a
  large photo will freeze the interface for a few seconds on a phone. Moving
  `Scrubber::save` onto a worker thread is the job to do before release, and it
  belongs in `native/` where both Qt platforms get it.
