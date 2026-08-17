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

# The Android build tools ship as bare executables everywhere except Windows,
# where the wrapper is a .bat. Naming the .bat unconditionally pinned this whole
# script to one Windows machine: it could not run on Linux, macOS, or CI, which
# also meant nobody else could rebuild an APK and compare it. That last part
# matters more than the convenience. The path remapping and zip-time.py in this
# same script exist to make the build reproducible, and reproducibility is
# worth exactly nothing if the build can only be reproduced by the one person
# who already has the original.
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) BAT=".bat" ;;
    *)                    BAT="" ;;
esac
# Debian and most distributions no longer ship a `python`, only `python3`.
PYTHON="$(command -v python3 || command -v python)"
: "${PYTHON:?need python3 for zip-time.py}"

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
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$(win "$CARGO_HOME")=/cargo --remap-path-prefix=$(win "$root")=/src"

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

# Check that the remapping above actually worked.
#
# build-desktop.sh has refused to call a build a release since the day the
# desktop leak was found. This script applied the same fix and then never
# looked, which left the one artefact that historically carried the author's
# home directory about ninety times as the only one nobody verified. A remap
# silently stops working if RUSTFLAGS is overridden in the environment, if
# CARGO_HOME moves, or if a dependency bakes in a path some other way, and none
# of those announce themselves.
echo "==> checking the library names no home directory"
leak=0
for n in ".cargo/registry" ".cargo\\registry" "/home/" "/Users/" "C:\\Users\\"; do
    found="$(grep -a -o -F -- "$n" "$out/apk/lib/$ABI/libmetascrub_android.so" 2>/dev/null | sort -u | head -2 || true)"
    if [ -n "$found" ]; then
        echo "    FAIL found $(printf '%s' "$found" | tr '\n' ' ')"
        leak=1
    fi
done
if [ "$leak" -ne 0 ]; then
    echo "    an APK that names its builder must not reach a handset" >&2
    exit 1
fi
echo "    ok"

# The version was typed here as `--version-code 1 --version-name 0.1.0`, in a
# second place that had to be kept in step with Cargo.toml by hand. Worse, a
# versionCode of 1 can never be updated: Android refuses to install a package
# whose code is not higher than the installed one, so every release after the
# first would have been rejected on the handset with a message about an
# existing app. Derived from the workspace version instead, which is the only
# place a version should live.
VERSION_NAME="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
[ -n "$VERSION_NAME" ] || { echo "could not read the version out of Cargo.toml" >&2; exit 1; }
# major*10000 + minor*100 + patch, so 0.1.0 is 100 and 1.2.3 is 10203. Ordered
# the same way the semantic version is, with room for 99 minors and patches.
VERSION_CODE="$(printf '%s' "$VERSION_NAME" | awk -F. '{print $1*10000 + $2*100 + $3}')"
echo "==> version $VERSION_NAME (code $VERSION_CODE)"

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
"$BT/d8$BAT" --min-api "$MIN_SDK" --release \
    --output "$(win "$out/dex")" \
    $(find "$out/classes" -name '*.class')

echo "==> package"
# Native libraries are stored (not deflated) and page-aligned so the app can
# load its own .so on newer Android; --no-compress plus `zipalign -p` does that.
cp "$out/base.apk" "$out/unaligned.apk"
cp "$out/dex/classes.dex" "$out/apk/classes.dex"
(cd "$out/apk" && "$JAVA_HOME/bin/jar" --update --no-compress \
    --file "$(win "$out/unaligned.apk")" classes.dex "lib")

echo "==> align"
"$BT/zipalign" -p -f 4 "$out/unaligned.apk" "$out/aligned.apk"

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
# `zipalign -p`'s page alignment of the .so survives untouched.
"$PYTHON" "$here/zip-time.py" "$out/aligned.apk"

echo "==> sign"
# A local debug key, generated on first run and never committed. A public
# release is signed with a key kept off this machine — see README.md.
KS="$here/debug.keystore"
if [ ! -f "$KS" ]; then
    "$JAVA_HOME/bin/keytool" -genkeypair -v \
        -keystore "$(win "$KS")" -storepass android -keypass android \
        -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 \
        -dname "CN=Crake Debug, OU=, O=, L=, S=, C=" >/dev/null
fi
"$BT/apksigner$BAT" sign \
    --ks "$(win "$KS")" --ks-pass pass:android --key-pass pass:android \
    --v1-signing-enabled false --v2-signing-enabled true --v3-signing-enabled true \
    --out "$out/metascrub-$ABI.apk" "$out/aligned.apk"
"$BT/apksigner$BAT" verify --print-certs "$out/metascrub-$ABI.apk" | head -3

# Leave only the signed APK(s) behind, not the staging tree.
rm -rf "$out/compiled" "$out/classes" "$out/dex" "$out/apk" "$out/gen" \
       "$out/base.apk" "$out/unaligned.apk" "$out/aligned.apk"

echo
echo "built: $out/metascrub-$ABI.apk"
