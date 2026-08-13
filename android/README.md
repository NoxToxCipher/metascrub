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
  $ANDROID_HOME/build-tools/34.0.0/aapt2 dump permissions build/metascrub-arm64-v8a.apk
  # prints the package name and nothing else — no <uses-permission>
  ```

- **Pure Rust core, so a tiny APK (~1 MB)** and no C libraries to trust or
  cross-compile. This is why the cross-compile "just works" where the messenger's
  (libsodium + c-toxcore) does not yet.

- **Nothing is logged**, and nothing is kept: the bytes live in memory for one
  screen; the only thing written is the cleaned copy the user saves.

## Build

Prerequisites are the same toolchain the Tox client uses, under
`C:/Users/user/android-tools` (JDK 17, the SDK with build-tools 34.0.0 and
platform android-34, and an NDK). Plus the Rust Android targets:

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

## Releasing (website download / Accrescent)

The build signs with a **debug** key (`android/debug.keystore`, generated on
first run, never committed) — fine for testing, not for a public release. For a
release:

1. Generate a release key **kept off this machine** and back it up — losing it
   means never being able to update the app for existing installs:

   ```bash
   keytool -genkeypair -v -keystore crake-release.jks -alias metascrub \
       -keyalg RSA -keysize 4096 -validity 10000
   ```

2. Re-sign the aligned APK with it (swap the `apksigner` line's `--ks` /
   passwords in `build-apk.sh`, or sign `build/aligned.apk` manually).

3. Publish the APK for direct download over the onion/site, with its SHA-256 so
   people can verify it. Sideloading needs "install unknown apps" enabled for the
   browser — the app is not in Play, by design.

4. **Accrescent** (optional, later): Accrescent wants its own signing and a short
   review; the zero-permission manifest and reproducible-ish direct-tools build
   are a good fit. This is a follow-up, not a blocker for a website download.

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
