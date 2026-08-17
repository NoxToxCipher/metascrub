# metascrub for Android

A share-sheet metadata scrubber. Share a photo, PDF or Office document to
**metascrub**, see what it carries, and save a cleaned copy — all on the device.

The same Rust core as the desktop, behind a thin JNI boundary
(`crates/metascrub-android`) and one Java activity. No Gradle, no libraries.

## What makes it a privacy app

- **Zero permissions.** The manifest requests none — not INTERNET, storage,
  location, or camera. It reads the file the share sheet hands it (a content URI
  the system grants for that one file) and writes the cleaned copy through the
  Storage Access Framework, which the user drives. The *absence* of INTERNET is
  verifiable proof the app cannot phone home:

  ```bash
  $ANDROID_HOME/build-tools/36.0.0/aapt2 dump permissions build/metascrub-arm64-v8a.apk
  # prints the package name and nothing else — no <uses-permission>
  ```

- **Pure Rust core, so a tiny APK (~1 MB)** and no C libraries to trust or
  cross-compile. This is why the cross-compile "just works" where the messenger's
  (libsodium + c-toxcore) does not yet.

- **Nothing is logged**, and nothing is kept: the bytes live in memory for one
  screen; the only thing written is the cleaned copy the user saves.

## Build

Prerequisites are the same toolchain the Tox client uses, under
`$HOME/android-tools` by default (JDK 17, the SDK with build-tools 36.0.0 and
platform android-36, and an NDK — r28 or newer for 16 KB pages); override
`ANDROID_TOOLS` to point elsewhere. Every SDK level, the version number and the
ABI list live in one file, `config.sh`, which both builds read.
Plus the Rust Android targets:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk   # once
```

Then, from the repo root (Git Bash):

```bash
source android/env.sh
android/build-apk.sh arm64-v8a       # or armeabi-v7a, x86_64, x86
# -> android/build/metascrub-<abi>.apk
```

`build-apk.sh` runs `cargo ndk` → `aapt2` → `javac` → `d8` → `zipalign` →
`apksigner` directly. There is no Gradle wrapper by design; see the header of the
script for why.

## Install and test (plug the phone in)

```bash
source android/env.sh
adb install -r android/build/metascrub-arm64-v8a.apk
```

Then on the phone: open a photo in the gallery → **Share → metascrub**. It shows
the findings (EXIF, GPS, maker note, thumbnail…) and lets you **Save cleaned
copy**. Confirm the saved file opens and carries no metadata.

`arm64-v8a` covers essentially every phone from ~2017 on (including the CMF
Phone 1). `armeabi-v7a` is built too, for older 32-bit devices.

## Releasing

Two shapes, built from the same source and the same `config.sh`, so they cannot
disagree about what version they are:

| | command | output | for |
|---|---|---|---|
| One APK per ABI | `build-apk.sh <abi>` | `build/metascrub-<abi>.apk` | direct download, a handset |
| An APK set | `build-bundle.sh` | `build/metascrub.apks` (and the `.aab`) | a store |

### The signing key

Without one, `build-apk.sh` signs with a **debug** key it generates on first run
(`android/debug.keystore`, never committed). That is right for a phone on the
desk and rejected by every store, which is what a key with a published password
deserves. Generate the real one once, keep it **off the build machine**, and back
it up — losing it means never being able to update the app for existing installs,
because Android identifies an app by its signature and nothing else:

```bash
keytool -genkeypair -v -keystore metascrub-release.jks -alias metascrub \
    -keyalg RSA -keysize 4096 -validity 10000
```

Both builds then take it from the environment. Leave the passwords out: the tools
ask at the terminal, so the passphrase never lands in a shell history or a
process listing.

```bash
export METASCRUB_KEYSTORE=/path/to/metascrub-release.jks
export METASCRUB_KEY_ALIAS=metascrub
android/build-bundle.sh
```

`build-bundle.sh` verifies what it produced before it says it worked: every APK
in the set is checked for 16 KB alignment and a valid signature, the set is
checked against the 128 MiB ceiling, and anything still carrying a build
timestamp is reported rather than quietly shipped.

### Direct download

Build each ABI, publish the APKs with their SHA-256 so people can check what they
downloaded. Sideloading needs "install unknown apps" enabled for the browser; the
app is not in Play, by design.

### Accrescent

Where the app actually stands against [their publishing
requirements](https://accrescent.app/docs/guide/publish/requirements.html):

| Requirement | State |
|---|---|
| APK set from bundletool 1.11.4+, at most 128 MiB | `build-bundle.sh`, ~1 MB |
| Signed v2/v3, never a debug certificate, one certificate only | release key, above |
| targetSdk tracks Google Play's floor (36 from 31 August 2026) | `config.sh` |
| `debuggable`, `testOnly`, `usesCleartextTraffic` all absent | manifest declares none |
| No non-standard update mechanism | there is no networking code at all |
| A 512×512 PNG icon and a listing | to do |
| Developer console account (GitHub login, currently allowlisted) | to do — access has to be requested |
| A domain matching the app ID, verified by a DNS record they send | **blocked**: `org.crake.metascrub` requires `crake.org` |

The app ID is the one thing here that cannot be changed later: it is the app's
permanent identity on the store and on every device that installs it. So the name
question and the domain settle first, and everything else waits behind them.

One thing worth deciding before publishing anywhere, not after: from September
2026 Google requires developers of apps installed on certified Android devices to
register a legal identity, a government ID and a signing key with it, starting in
Brazil, Indonesia, Singapore and Thailand and spreading through 2027. It applies
to sideloading and to third-party stores, not only Play. Accrescent itself is
[largely unaffected](https://blog.accrescent.app/posts/android-developer-verification/)
because it always distributed developer-signed apps, but the burden lands on the
developer, and this project is deliberately pseudonymous.

## What is deliberately minimal in v1

- One ABI's worth of testing so far (arm64); armv7 builds, untested on a 32-bit
  device.
- **Save** goes through the Storage Access Framework. **Share cleaned copy** (to
  another app) is not wired yet — it needs a small `ContentProvider`, kept out of
  v1 to avoid pulling in `androidx`.
- A file is cleaned entirely in memory (and copied again across the JNI
  boundary), so there is a **100 MB cap** — above a large RAW photo, below what
  would risk an out-of-memory crash. Streaming very large files is a later job.
- The report is rendered as plain text. It could grow the desktop's coloured
  badges later; the JSON from `Native.reportJson` already carries everything.
- `reportJson` and `sanitize` each parse the file once, so a Save re-parses. Fine
  for photos; a single-pass combined call is the obvious later optimisation.
