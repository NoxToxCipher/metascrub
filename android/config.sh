# The numbers every part of the Android build has to agree on.
#
#   . android/config.sh
#
# One place for the SDK levels, the version, and the pinned tool versions, so
# the APK, the bundle and the native library cannot be built against different
# ones.

# The oldest Android this app runs on. metascrub is a pure-Rust core with no C
# libraries, so — unlike the messenger — there is no second number to keep in
# step; the NDK builds the one `.so` against the platform default and it runs
# everywhere this level does.
MIN_SDK=26

# What the app is built and tested against.
#
# 36 rather than a comfortable older number because a store sets the floor, not
# us: Accrescent follows Google Play's target API policy, and from 31 August
# 2026 a newly submitted app must target 36. Two things follow from it, both
# handled in build-apk.sh and build-bundle.sh rather than left to be discovered
# on a handset:
#
#   * 16 KB memory pages. An app targeting 35+ has to load its native library on
#     devices whose page size is 16 KB, which means the .so is linked with
#     max-page-size=16384 and the archive is aligned with `zipalign -P 16`.
#   * Edge to edge. Android 15 lays every app out under the status and
#     navigation bars and ignores the theme colours for them, so the activity
#     reads the window insets and pads by them (MainActivity.applyBarInsets).
TARGET_SDK=36

# The build-tools this repository is pinned to. Named rather than globbed, so a
# second version appearing in the SDK cannot silently change what ships. 35.0.0
# is the floor for the 16 KB work: `zipalign -P` does not exist before it.
BUILD_TOOLS=36.0.0

# What a release calls itself. Both builds read these, so a bundle and an APK
# built from the same commit cannot disagree about which version they are.
#
# versionCode must increase for every upload Accrescent accepts, and it is the
# only number a device compares when deciding whether an update is newer.
VERSION_CODE=1
VERSION_NAME=0.1.0

# The ABIs a release covers. arm64 is every phone from roughly 2017 on; armv7 is
# there for older 32-bit hardware and is still untested on a real one. The x86
# pair is emulator territory and is left out of a release on purpose: a split
# nobody installs is a split nobody has tested.
ABIS="${ABIS:-arm64-v8a armeabi-v7a}"

# Accrescent's floor for the tool that produces the upload. Checked at run time
# against `bundletool version`, because a jar can be named anything.
BUNDLETOOL_MIN=1.11.4
