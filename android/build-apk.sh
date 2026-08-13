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
    --version-code 1 --version-name 0.1.0 \
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
"$BT/d8.bat" --min-api "$MIN_SDK" --release \
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
"$BT/apksigner.bat" sign \
    --ks "$(win "$KS")" --ks-pass pass:android --key-pass pass:android \
    --v1-signing-enabled false --v2-signing-enabled true --v3-signing-enabled true \
    --out "$out/metascrub-$ABI.apk" "$out/aligned.apk"
"$BT/apksigner.bat" verify --print-certs "$out/metascrub-$ABI.apk" | head -3

# Leave only the signed APK(s) behind, not the staging tree.
rm -rf "$out/compiled" "$out/classes" "$out/dex" "$out/apk" "$out/gen" \
       "$out/base.apk" "$out/unaligned.apk" "$out/aligned.apk"

echo
echo "built: $out/metascrub-$ABI.apk"
