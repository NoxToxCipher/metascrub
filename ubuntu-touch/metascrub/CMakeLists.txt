# metascrub for Ubuntu Touch — a native Lomiri app over the pure-Rust core.
#
# The Rust core is compiled to a C-ABI static library (crate metascrub-ffi) for
# the target architecture by ubuntu-touch/build-ffi.sh, then linked straight into
# the app binary, so the click ships one executable and no loose .so to find at
# runtime.
#
# Everything installs relative to CMAKE_INSTALL_PREFIX, which for a click build
# is the directory that becomes the package root:
#
#   /metascrub                    the binary named by the desktop file's Exec
#   /qml/...                      the interface
#   /assets/logo.svg              the launcher icon
#   /manifest.json /metascrub.apparmor /metascrub.desktop /metascrub-contenthub.json

cmake_minimum_required(VERSION 3.10)
project(metascrub CXX C)

set(APP_NAME metascrub)
set(APP_ID "metascrub.noxtoxcipher")
set(APP_HOOK "metascrub")
set(APP_VERSION "0.1.0")
set(CLICK_FRAMEWORK "ubuntu-sdk-20.04")

set(CMAKE_CXX_STANDARD 14)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_INCLUDE_CURRENT_DIR ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

find_package(Qt5 5.9 REQUIRED COMPONENTS Core Gui Qml Quick)

# --- where things are -------------------------------------------------------
# The repository root, two levels up. The Qt backend and the C header live there
# and are shared with the Sailfish app.
get_filename_component(REPO_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
set(METASCRUB_NATIVE_DIR "${REPO_ROOT}/native")
set(METASCRUB_FFI_INCLUDE "${REPO_ROOT}/crates/metascrub-ffi/include")
# The Handbook text is maintained once, in the Android tree, and copied in at
# build time rather than duplicated into a third file that can go stale.
set(HANDBOOK_SOURCE "${REPO_ROOT}/android/app/src/main/res/raw/handbook.json")

# --- architecture -----------------------------------------------------------
# Ask the compiler that is actually going to build this, so a cross build in the
# clickable container reports the target and not the host.
execute_process(COMMAND ${CMAKE_CXX_COMPILER} -dumpmachine
                OUTPUT_VARIABLE ARCH_TRIPLET
                OUTPUT_STRIP_TRAILING_WHITESPACE
                ERROR_QUIET)
if(NOT ARCH_TRIPLET)
    set(ARCH_TRIPLET "x86_64-linux-gnu")
endif()

if(ARCH_TRIPLET MATCHES "^aarch64")
    set(CLICK_ARCH "arm64")
elseif(ARCH_TRIPLET MATCHES "^arm")
    set(CLICK_ARCH "armhf")
elseif(ARCH_TRIPLET MATCHES "^x86_64")
    set(CLICK_ARCH "amd64")
else()
    message(FATAL_ERROR "unsupported architecture triplet '${ARCH_TRIPLET}'")
endif()
message(STATUS "metascrub: ${CLICK_ARCH} (${ARCH_TRIPLET})")

# --- the Rust core ----------------------------------------------------------
find_library(METASCRUB_FFI_LIB
    NAMES metascrub_ffi
    HINTS
        ${METASCRUB_LIB_DIR}
        ${CMAKE_CURRENT_SOURCE_DIR}/rustlib/${ARCH_TRIPLET}
    NO_DEFAULT_PATH)
if(NOT METASCRUB_FFI_LIB)
    message(FATAL_ERROR
        "libmetascrub_ffi not found for ${ARCH_TRIPLET}.\n"
        "Build it first:  ubuntu-touch/build-ffi.sh ${CLICK_ARCH}\n"
        "or point the build at an existing one:  -DMETASCRUB_LIB_DIR=<dir>")
endif()
message(STATUS "metascrub: core at ${METASCRUB_FFI_LIB}")

# --- the app ----------------------------------------------------------------
add_executable(${APP_NAME}
    src/main.cpp
    src/workspace.cpp
    src/workspace.h
    ${METASCRUB_NATIVE_DIR}/scrubber.cpp
    ${METASCRUB_NATIVE_DIR}/scrubber.h)

target_include_directories(${APP_NAME} PRIVATE
    ${METASCRUB_NATIVE_DIR}
    ${METASCRUB_FFI_INCLUDE})

# The Rust staticlib brings its own libstd, which needs these at link time.
target_link_libraries(${APP_NAME} PRIVATE
    ${METASCRUB_FFI_LIB}
    ${CMAKE_DL_LIBS}
    pthread
    m
    Qt5::Core Qt5::Gui Qt5::Qml Qt5::Quick)

# --- packaging --------------------------------------------------------------
configure_file(manifest.json.in ${CMAKE_CURRENT_BINARY_DIR}/manifest.json @ONLY)
configure_file(${APP_HOOK}.desktop.in ${CMAKE_CURRENT_BINARY_DIR}/${APP_HOOK}.desktop @ONLY)

install(TARGETS ${APP_NAME} RUNTIME DESTINATION ".")
install(DIRECTORY qml DESTINATION "." PATTERN "*.in" EXCLUDE)
install(DIRECTORY assets DESTINATION ".")
install(FILES ${HANDBOOK_SOURCE} DESTINATION "qml")
install(FILES
    ${CMAKE_CURRENT_BINARY_DIR}/manifest.json
    ${CMAKE_CURRENT_BINARY_DIR}/${APP_HOOK}.desktop
    ${CMAKE_CURRENT_SOURCE_DIR}/${APP_HOOK}.apparmor
    ${CMAKE_CURRENT_SOURCE_DIR}/${APP_HOOK}-contenthub.json
    DESTINATION ".")

# --- tests ------------------------------------------------------------------
# Off by default so a phone build stays lean; CI turns it on for amd64. It runs
# a real file through the real Rust library over the real C ABI, which is the
# part of this app that must not silently break.
option(METASCRUB_TESTS "Build the native smoke test" OFF)
if(METASCRUB_TESTS)
    enable_testing()
    add_executable(scrubber_smoke
        tests/scrubber_smoke.cpp
        ${METASCRUB_NATIVE_DIR}/scrubber.cpp
        ${METASCRUB_NATIVE_DIR}/scrubber.h)
    target_include_directories(scrubber_smoke PRIVATE
        ${METASCRUB_NATIVE_DIR}
        ${METASCRUB_FFI_INCLUDE})
    target_link_libraries(scrubber_smoke PRIVATE
        ${METASCRUB_FFI_LIB} ${CMAKE_DL_LIBS} pthread m Qt5::Core Qt5::Gui)
    add_test(NAME scrubber_smoke COMMAND scrubber_smoke)
endif()
