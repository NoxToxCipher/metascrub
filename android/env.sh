# Source this before building the Android side.
#
#   source android/env.sh
#   android/build-apk.sh [abi]
#
# Reuses the toolchain the sibling Tox client already installed under
# $HOME/android-tools (JDK 17, the SDK, and an NDK). Override ANDROID_TOOLS in
# your environment if you keep it elsewhere; nothing else hard-codes the path.

export ANDROID_TOOLS="${ANDROID_TOOLS:-$HOME/android-tools}"
export JAVA_HOME="$ANDROID_TOOLS/jdk-17.0.20+8"
export ANDROID_HOME="$ANDROID_TOOLS/sdk"
export ANDROID_SDK_ROOT="$ANDROID_HOME"

# Resolved rather than hard-coded, so installing a newer NDK does not silently
# leave this pointing at the old one.
export ANDROID_NDK_HOME="$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1)"
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"

export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"

# bundletool, which turns the build into the .apks set a store takes. It is a
# single jar Google publishes on GitHub rather than an SDK package, so it is not
# installed by sdkmanager and has to be dropped in by hand:
#
#   https://github.com/google/bundletool/releases -> bundletool-all-<ver>.jar
#   mv bundletool-all-*.jar "$ANDROID_TOOLS/bundletool.jar"
#
# Only build-bundle.sh needs it; the plain APK build does not, so a missing jar
# is reported there and not here.
export BUNDLETOOL_JAR="${BUNDLETOOL_JAR:-$ANDROID_TOOLS/bundletool.jar}"

echo "  JAVA_HOME        $JAVA_HOME"
echo "  ANDROID_HOME     $ANDROID_HOME"
echo "  ANDROID_NDK_HOME ${ANDROID_NDK_HOME:-NOT INSTALLED}"
echo "  BUNDLETOOL_JAR   $BUNDLETOOL_JAR$([ -f "$BUNDLETOOL_JAR" ] || echo '  (not installed)')"
