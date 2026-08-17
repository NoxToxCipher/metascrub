#!/usr/bin/env bash
#
# Build the store upload: an app bundle, and the APK set derived from it.
#
#   source android/env.sh
#   METASCRUB_KEYSTORE=/path/to/release.jks METASCRUB_KEY_ALIAS=metascrub \
#       android/build-bundle.sh
#
#   -> android/build/metascrub.aab    the bundle
#   -> android/build/metascrub.apks   the file a store takes
#
# ## Why this exists next to build-apk.sh
#
# build-apk.sh makes one APK per ABI, which is what a direct download and a
# handset want: one file, install it, done. A store wants the other shape — a
# bundle it splits so a phone downloads only its own architecture and its own
# screen density. Accrescent takes an APK set (.apks) produced by bundletool,
# and will not take a bare APK.
#
# Both builds read android/config.sh, so the two cannot disagree about SDK
# levels, version or ABIs. The steps up to the dex are deliberately the same
# commands; the difference is one aapt2 flag (--proto-format, which is what a
# bundle module holds instead of a binary manifest) and what happens after.
#
# ## Why no Gradle, still
#
# A bundle is a zip in a documented layout plus a bundletool invocation. Adding
# a build system that downloads a second build system to produce it would be a
# large new dependency surface for a project whose whole claim is that you can
# read what it does.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(dirname "$here")"
app="$here/app/src/main"
out="$here/build"
: "${ANDROID_HOME:?source android/env.sh first}"
: "${ANDROID_NDK_HOME:?source android/env.sh first}"
: "${JAVA_HOME:?source android/env.sh first}"
: "${BUNDLETOOL_JAR:?source android/env.sh first}"
. "$here/config.sh"

BT="$ANDROID_HOME/build-tools/$BUILD_TOOLS"
JAR="$ANDROID_HOME/platforms/android-$TARGET_SDK/android.jar"

# Windows paths for the Java/Windows tools, POSIX for the shell.
win() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else printf '%s' "$1"; fi; }

# A .bat on Windows, an extensionless script elsewhere. Same tool.
bt() {
    local name="$1"; shift
    if [ -f "$BT/$name.bat" ]; then "$BT/$name.bat" "$@"; else "$BT/$name" "$@"; fi
}

bundletool() { "$JAVA_HOME/bin/java" -jar "$(win "$BUNDLETOOL_JAR")" "$@"; }

# python is called "python" on the Windows box this is usually built on and
# "python3" on most Linux ones; zip-time.py runs under either.
PY="${PYTHON:-python}"
command -v "$PY" >/dev/null || PY=python3

# Python is already a build dependency (zip-time.py) and is the one unzip that
# is definitely present on every machine this builds on; Git Bash on Windows
# ships no unzip at all.
unzip_to() {
    "$PY" -c \
        "import zipfile,sys; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])" "$1" "$2"
}

# ---------------------------------------------------------------- preflight

# A release key, not the debug one. Accrescent rejects debug certificates
# outright, and it is right to: the password is public and the key sits on the
# build machine, so anyone could publish an "update" to your app.
: "${METASCRUB_KEYSTORE:?a store upload needs the release key: set METASCRUB_KEYSTORE (and METASCRUB_KEY_ALIAS)}"
: "${METASCRUB_KEY_ALIAS:?set METASCRUB_KEY_ALIAS alongside METASCRUB_KEYSTORE}"
[ -f "$METASCRUB_KEYSTORE" ] || { echo "no such keystore: $METASCRUB_KEYSTORE" >&2; exit 1; }
case "$METASCRUB_KEYSTORE" in
    *debug.keystore) echo "that is the debug key; a store will reject it" >&2; exit 1 ;;
esac

[ -f "$BUNDLETOOL_JAR" ] || {
    echo "no bundletool at $BUNDLETOOL_JAR" >&2
    echo "download bundletool-all-<version>.jar from" >&2
    echo "  https://github.com/google/bundletool/releases" >&2
    echo "and save it there (or set BUNDLETOOL_JAR)." >&2
    exit 1
}

# Accrescent sets a floor on the tool, because older versions produce sets it
# cannot read. A jar can be named anything, so ask the jar itself.
have="$(bundletool version | tr -d '\r' | head -1)"
if [ "$(printf '%s\n%s\n' "$have" "$BUNDLETOOL_MIN" | sort -V | head -1)" != "$BUNDLETOOL_MIN" ]; then
    echo "bundletool $have is older than the required $BUNDLETOOL_MIN" >&2
    exit 1
fi
echo "==> bundletool $have, signing with $METASCRUB_KEY_ALIAS"

rm -rf "$out/compiled" "$out/classes" "$out/dex" "$out/gen" "$out/module" \
       "$out/base-proto.zip" "$out/base.zip" "$out/verify" \
       "$out/metascrub.aab" "$out/metascrub.apks"
mkdir -p "$out/compiled" "$out/classes" "$out/dex" "$out/module"

# ------------------------------------------------------------------- native

echo "==> native libraries ($ABIS)"
export CARGO_TARGET_DIR="$root/target-android"
: "${CARGO_HOME:=$HOME/.cargo}"

# Both halves of the 16 KB page story, as in build-apk.sh: the library is linked
# for 16 KB pages here, and bundletool is asked to keep it uncompressed so the
# device can map it in place. The alignment of what comes out is verified at the
# end rather than trusted.
export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-z,max-page-size=16384 --remap-path-prefix=$(win "$CARGO_HOME")=/cargo --remap-path-prefix=$(win "$root")=/src"

ndk_targets=()
for abi in $ABIS; do ndk_targets+=(-t "$abi"); done
(cd "$root" && cargo ndk "${ndk_targets[@]}" -o "$(win "$out/module/lib")" build --release -p metascrub-android)

# A whitelist, not a blocklist: only the library MainActivity loads is shipped.
for abi in $ABIS; do
    [ -d "$out/module/lib/$abi" ] || { echo "cargo ndk produced no $abi library" >&2; exit 1; }
    for so in "$out/module/lib/$abi"/*.so; do
        [ -f "$so" ] || continue
        case "$(basename "$so")" in
            libmetascrub_android.so) echo "    $abi/$(basename "$so")" ;;
            *) echo "    not shipping $abi/$(basename "$so") - nothing loads it"; rm -f "$so" ;;
        esac
    done
done

# ---------------------------------------------------------------- resources

echo "==> resources (proto)"
"$BT/aapt2" compile --dir "$app/res" -o "$out/compiled/res.zip"

# --proto-format is the whole difference from build-apk.sh: a bundle module
# carries the manifest and the resource table as protocol buffers, because
# bundletool has to rewrite them when it splits the app up.
"$BT/aapt2" link \
    --proto-format \
    -o "$out/base-proto.zip" \
    -I "$JAR" \
    --manifest "$app/AndroidManifest.xml" \
    --java "$out/gen" \
    --min-sdk-version "$MIN_SDK" --target-sdk-version "$TARGET_SDK" \
    --version-code "$VERSION_CODE" --version-name "$VERSION_NAME" \
    "$out/compiled/res.zip"

echo "==> java"
"$JAVA_HOME/bin/javac" --release 17 -encoding UTF-8 \
    -classpath "$(win "$JAR")" \
    -d "$(win "$out/classes")" \
    $(find "$app/java" -name '*.java') \
    $(find "$out/gen" -name '*.java')

echo "==> dex"
bt d8 --min-api "$MIN_SDK" --release \
    --output "$(win "$out/dex")" \
    $(find "$out/classes" -name '*.class')

# ------------------------------------------------------------------- bundle

echo "==> module"
# A bundle module is a zip in a fixed layout: the manifest under manifest/, the
# resource table at the root, dex under dex/, native libraries under lib/<abi>/.
# aapt2 emits the first three of those in a flat zip, so they are moved into
# place rather than rebuilt.
unzip_to "$out/base-proto.zip" "$out/module"
mkdir -p "$out/module/manifest" "$out/module/dex"
mv "$out/module/AndroidManifest.xml" "$out/module/manifest/AndroidManifest.xml"
cp "$out/dex/classes.dex" "$out/module/dex/classes.dex"
[ -f "$out/module/resources.pb" ] || { echo "aapt2 wrote no resources.pb — is --proto-format supported by build-tools $BUILD_TOOLS?" >&2; exit 1; }

# jar rather than zip: Git Bash ships no zip either, and --no-manifest keeps the
# META-INF directory jar would otherwise add out of a tree bundletool validates.
rm -f "$out/base.zip"
"$JAVA_HOME/bin/jar" --create --file "$(win "$out/base.zip")" --no-manifest \
    -C "$(win "$out/module")" .

echo "==> bundle"
bundletool build-bundle \
    --modules="$(win "$out/base.zip")" \
    --output="$(win "$out/metascrub.aab")" \
    --overwrite

echo "==> apk set"
# The passwords are asked for at the terminal unless the environment supplies
# them, so a release passphrase does not have to live in a shell history or a
# process listing to build a release.
apks_args=(--ks="$(win "$METASCRUB_KEYSTORE")" --ks-key-alias="$METASCRUB_KEY_ALIAS")
if [ -n "${METASCRUB_KS_PASS:-}" ]; then apks_args+=(--ks-pass="$METASCRUB_KS_PASS"); fi
if [ -n "${METASCRUB_KEY_PASS:-}" ]; then apks_args+=(--key-pass="$METASCRUB_KEY_PASS"); fi
bundletool build-apks \
    --bundle="$(win "$out/metascrub.aab")" \
    --output="$(win "$out/metascrub.apks")" \
    --overwrite \
    "${apks_args[@]}"

# The wrapper's own entry timestamps are this machine's local clock, same as any
# other zip. The APKs inside it are bundletool's and are signed, so they are
# reported below rather than patched.
echo "==> flatten timestamps (wrapper)"
"$PY" "$here/zip-time.py" "$out/metascrub.apks"

# ------------------------------------------------------------------- verify

echo "==> verify"
bytes="$(wc -c < "$out/metascrub.apks" | tr -d ' ')"
limit=$((128 * 1024 * 1024))
printf '    set size %s bytes (limit %s)\n' "$bytes" "$limit"
[ "$bytes" -le "$limit" ] || { echo "    the set is over the 128 MiB a store accepts" >&2; exit 1; }

rm -rf "$out/verify"
unzip_to "$out/metascrub.apks" "$out/verify"
found=0
while IFS= read -r split; do
    found=$((found + 1))
    name="$(basename "$split")"
    # 16 KB alignment, per split, because only the ABI splits carry a library
    # and a misaligned one fails at System.loadLibrary on a 16 KB device.
    "$BT/zipalign" -c -P 16 4 "$split" >/dev/null \
        || { echo "    $name is NOT 16 KB aligned" >&2; exit 1; }
    # Signed with the release key, v2/v3, which is what a store checks.
    bt apksigner verify --min-sdk-version "$MIN_SDK" "$split" \
        || { echo "    $name failed signature verification" >&2; exit 1; }
    echo "    $name: aligned, signed"
done < <(find "$out/verify" -name '*.apk' | sort)
[ "$found" -gt 0 ] || { echo "    the set contains no APKs" >&2; exit 1; }

# Honest about what was not fixed: bundletool writes and signs these, so their
# timestamps cannot be flattened afterwards without breaking the signature.
# Reported rather than left to be assumed either way.
stamped=0
while IFS= read -r split; do
    "$PY" "$here/zip-time.py" --check "$split" >/dev/null 2>&1 || stamped=$((stamped + 1))
done < <(find "$out/verify" -name '*.apk' | sort)
if [ "$stamped" -gt 0 ]; then
    echo "    note: $stamped of $found APKs carry bundletool's build timestamps"
    echo "          (the wrapper is flattened; the splits are signed, so they cannot be)"
else
    echo "    no APK in the set carries a build timestamp"
fi

bt apksigner verify --print-certs "$(find "$out/verify" -name '*.apk' | sort | head -1)" | head -3

rm -rf "$out/compiled" "$out/classes" "$out/dex" "$out/gen" "$out/module" \
       "$out/base-proto.zip" "$out/base.zip" "$out/verify"

echo
echo "built: $out/metascrub.aab"
echo "       $out/metascrub.apks   (version $VERSION_NAME, code $VERSION_CODE)"
echo
echo "test it on a connected phone with:"
echo "  java -jar \"\$BUNDLETOOL_JAR\" install-apks --apks=$out/metascrub.apks"
