#!/usr/bin/env bash
#
# Build the metascrub APK without Gradle.
#
#   source android/env.sh
#   android/build-apk.sh [abi]
#
# ## Why there is no Gradle here
#
# The same reasoning as the sibling Tox client: one activity, no libraries, no
# dependencies to resolve — so the Android build tools are called directly
# rather than through a wrapper that downloads a build system on first run.
#
#   cargo ndk -> aapt2 compile -> aapt2 link -> javac -> d8 -> zip -> zipalign -> apksigner
#
# ## Why this one just works where the messenger's does not
#
# metascrub is a pure-Rust core: zune-jpeg, png, image-webp, miniz_oxide, lopdf,
# jni — no C libraries to cross-compile first. The `cargo ndk` step below links
# cleanly, so this produces a runnable APK today.
set -euo pipefail

ABI="${1:-arm64-v8a}"
case "$ABI" in
    arm64-v8a|armeabi-v7a|x86_64|x86) ;;
    *) echo "unknown ABI: $ABI" >&2; exit 2 ;;
esac

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(dirname "$here")"
app="$here/app/src/main"
out="$here/build"
: "${ANDROID_HOME:?source android/env.sh first}"
: "${ANDROID_NDK_HOME:?source android/env.sh first}"
: "${JAVA_HOME:?source android/env.sh first}"
. "$here/config.sh"

BT="$ANDROID_HOME/build-tools/$BUILD_TOOLS"
JAR="$ANDROID_HOME/platforms/android-$TARGET_SDK/android.jar"

# Windows paths for the Java/Windows tools, POSIX for the shell.
win() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else printf '%s' "$1"; fi; }

# Some build tools are a Java launcher: a .bat on Windows and an extensionless
# shell script everywhere else, both in the same directory. Pick whichever is
# actually there, so the same script runs on a Linux box (and one day in CI)
# without a second copy of the build that can drift from this one.
bt() {
    local name="$1"; shift
    if [ -f "$BT/$name.bat" ]; then "$BT/$name.bat" "$@"; else "$BT/$name" "$@"; fi
}

# python is called "python" on the Windows box this is usually built on and
# "python3" on most Linux ones; zip-time.py runs under either.
PY="${PYTHON:-python}"
command -v "$PY" >/dev/null || PY=python3

# Clean this build's intermediates and this ABI's own previous APK, but leave
# any other ABI's finished APK in place — a release ships several, and they are
# built one `build-apk.sh <abi>` at a time.
rm -rf "$out/compiled" "$out/classes" "$out/dex" "$out/apk" "$out/gen" \
       "$out/base.apk" "$out/unaligned.apk" "$out/aligned.apk" \
       "$out/metascrub-$ABI.apk"
mkdir -p "$out/compiled" "$out/classes" "$out/dex" "$out/apk/lib/$ABI"

echo "==> native library ($ABI)"
# A target directory of its own, so an Android build and a desktop build do not
# block each other on cargo's lock of target/.
export CARGO_TARGET_DIR="$root/target-android"

# Strip the build machine out of the binary.
#
# `strip = true` in Cargo.toml removes symbols, but panic locations are static
# strings in .rodata and survive it. Shipped APKs carried the absolute path of
# every panicking source file, so the author's home directory -- and username --
# appeared about ninety times inside a tool built to remove exactly that kind of
# trace from other people's files. Anyone could read it with `strings`.
#
# It also defeated rust-toolchain.toml: pinning the compiler is only half of a
# reproducible build, because two people with the same compiler still emit
# different bytes when their home directories have different names. Remapping
# gives every machine the same strings, so the hashes can actually match.
#
# `trim-paths` in [profile.release] is the tidy form of this, but it is not
# stable in Cargo 1.97.1 and this workspace pins stable on purpose.
: "${CARGO_HOME:=$HOME/.cargo}"

# Link the library for 16 KB memory pages.
#
# Android has always had 4 KB pages; newer arm64 devices have 16 KB ones, and an
# app targeting 35 or above is expected to load on both. A .so laid out for 4 KB
# pages cannot be mapped by a kernel using 16 KB ones, so the app dies at
# System.loadLibrary with a message about the library not being page-aligned.
#
# NDK r28 and later already link this way, but the flag is passed explicitly
# rather than assumed, because the NDK here is whichever one env.sh resolved and
# a build that quietly depends on the toolchain version is a build that breaks
# on someone else's machine. `zipalign -P 16` further down does the other half:
# the library must also sit on a 16 KB boundary inside the archive.
export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-z,max-page-size=16384 --remap-path-prefix=$(win "$CARGO_HOME")=/cargo --remap-path-prefix=$(win "$root")=/src"

# cargo-ndk writes the stripped .so straight into the staging tree.
(cd "$root" && cargo ndk -t "$ABI" -o "$(win "$out/apk/lib")" build --release -p metascrub-android)

# A whitelist, not a blocklist: a new cdylib in the workspace must be named here
# to reach a handset. Only the library MainActivity loads is shipped.
for so in "$out/apk/lib/$ABI"/*.so; do
    [ -f "$so" ] || continue
    case "$(basename "$so")" in
        libmetascrub_android.so) ;;
        *) echo "    not shipping $(basename "$so") - nothing loads it"; rm -f "$so" ;;
    esac
done
ls -1 "$out/apk/lib/$ABI/" | sed 's/^/    /'

echo "==> resources"
"$BT/aapt2" compile --dir "$app/res" -o "$out/compiled/res.zip"
"$BT/aapt2" link \
    -o "$out/base.apk" \
    -I "$JAR" \
    --manifest "$app/AndroidManifest.xml" \
    --java "$out/gen" \
    --min-sdk-version "$MIN_SDK" --target-sdk-version "$TARGET_SDK" \
    --version-code "$VERSION_CODE" --version-name "$VERSION_NAME" \
    "$out/compiled/res.zip"

echo "==> java"
# -encoding UTF-8 is not optional: the sources are UTF-8, and without it javac
# falls back to the platform encoding (Cp1252 on Windows) and silently corrupts
# every non-ASCII character in a string literal.
"$JAVA_HOME/bin/javac" --release 17 -encoding UTF-8 \
    -classpath "$(win "$JAR")" \
    -d "$(win "$out/classes")" \
    $(find "$app/java" -name '*.java') \
    $(find "$out/gen" -name '*.java')

echo "==> dex"
bt d8 --min-api "$MIN_SDK" --release \
    --output "$(win "$out/dex")" \
    $(find "$out/classes" -name '*.class')

echo "==> package"
# Native libraries are stored (not deflated) and page-aligned so the app can
# load its own .so straight out of the archive; --no-compress plus `zipalign -P`
# does that, and the manifest's extractNativeLibs="false" is what asks for it.
cp "$out/base.apk" "$out/unaligned.apk"
cp "$out/dex/classes.dex" "$out/apk/classes.dex"
(cd "$out/apk" && "$JAVA_HOME/bin/jar" --update --no-compress \
    --file "$(win "$out/unaligned.apk")" classes.dex "lib")

echo "==> align"
# -P 16 places the stored .so on a 16 KB boundary, which is what a device with
# 16 KB pages needs in order to map it without extracting it first. It replaces
# the older `-p` (which means 4 KB) and needs build-tools 35 or newer.
"$BT/zipalign" -P 16 -f 4 "$out/unaligned.apk" "$out/aligned.apk"

# Checked, not assumed. A wrongly aligned library is invisible until the app
# starts on a 16 KB device, which is exactly the kind of failure that reaches a
# user before it reaches us.
"$BT/zipalign" -c -P 16 4 "$out/aligned.apk"

echo "==> flatten timestamps"
# ZIP records modification times as MS-DOS date/time, which has no timezone
# field: every entry carried this machine's LOCAL clock. That does not just say
# when the APK was built, it says when it was built *where the builder is* --
# subtract it from any UTC reference and you have the build machine's offset,
# which for a pseudonymous project is a rough location given away by a field
# nobody reads. It was also the last thing stopping the APK being reproducible.
#
# Between align and sign, and in that order for two reasons: the v2/v3
# signature covers these bytes, and the patcher edits them in place so
# `zipalign -P 16`'s page alignment of the .so survives untouched.
"$PY" "$here/zip-time.py" "$out/aligned.apk"

echo "==> sign"
# By default a local debug key, generated on first run and never committed:
# enough to put the app on a handset, and rejected by any store, which is the
# correct behaviour for a key that lives on the build machine.
#
# Set METASCRUB_KEYSTORE and METASCRUB_KEY_ALIAS to sign with the real release
# key instead (for a direct download; a store upload goes through
# build-bundle.sh). The passwords are deliberately NOT read from the
# environment: leave them unset and apksigner asks at the terminal, so the
# release passphrase never sits in a shell history, a process listing or a
# .bash_profile. METASCRUB_KS_PASS / METASCRUB_KEY_PASS exist for an unattended
# build and take apksigner's own syntax (`file:...`, `env:VAR`, `pass:...`).
sign_args=(--v1-signing-enabled false --v2-signing-enabled true --v3-signing-enabled true)
if [ -n "${METASCRUB_KEYSTORE:-}" ]; then
    : "${METASCRUB_KEY_ALIAS:?set METASCRUB_KEY_ALIAS alongside METASCRUB_KEYSTORE}"
    [ -f "$METASCRUB_KEYSTORE" ] || { echo "no such keystore: $METASCRUB_KEYSTORE" >&2; exit 1; }
    echo "    release key: $METASCRUB_KEYSTORE ($METASCRUB_KEY_ALIAS)"
    sign_args+=(--ks "$(win "$METASCRUB_KEYSTORE")" --ks-key-alias "$METASCRUB_KEY_ALIAS")
    if [ -n "${METASCRUB_KS_PASS:-}" ]; then sign_args+=(--ks-pass "$METASCRUB_KS_PASS"); fi
    if [ -n "${METASCRUB_KEY_PASS:-}" ]; then sign_args+=(--key-pass "$METASCRUB_KEY_PASS"); fi
else
    KS="$here/debug.keystore"
    if [ ! -f "$KS" ]; then
        "$JAVA_HOME/bin/keytool" -genkeypair -v \
            -keystore "$(win "$KS")" -storepass android -keypass android \
            -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 \
            -dname "CN=Crake Debug, OU=, O=, L=, S=, C=" >/dev/null
    fi
    echo "    DEBUG key — for a handset, never for a release"
    sign_args+=(--ks "$(win "$KS")" --ks-pass pass:android --key-pass pass:android)
fi
bt apksigner sign "${sign_args[@]}" --out "$out/metascrub-$ABI.apk" "$out/aligned.apk"
bt apksigner verify --print-certs "$out/metascrub-$ABI.apk" | head -3

# Leave only the signed APK(s) behind, not the staging tree.
rm -rf "$out/compiled" "$out/classes" "$out/dex" "$out/apk" "$out/gen" \
       "$out/base.apk" "$out/unaligned.apk" "$out/aligned.apk"

echo
echo "built: $out/metascrub-$ABI.apk"
