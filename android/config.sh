# The numbers every part of the Android build has to agree on.
#
#   . android/config.sh
#
# One place for the SDK levels and pinned tool versions, so the APK and the
# native library cannot be built against different ones.

# The oldest Android this app runs on. metascrub is a pure-Rust core with no C
# libraries, so — unlike the messenger — there is no second number to keep in
# step; the NDK builds the one `.so` against the platform default and it runs
# everywhere this level does.
MIN_SDK=26

# What the app is built and tested against.
TARGET_SDK=34

# The build-tools this repository is pinned to. Named rather than globbed, so a
# second version appearing in the SDK cannot silently change what ships.
BUILD_TOOLS=34.0.0
