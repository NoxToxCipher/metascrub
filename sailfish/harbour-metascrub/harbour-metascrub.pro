# metascrub for Sailfish OS — a native Silica app over the pure-Rust core.
#
# The Rust core is compiled to a C-ABI static library (crate metascrub-ffi) for
# the Sailfish target, then linked here. The RPM spec's %build runs cargo first
# and passes RUST_LIB_DIR pointing at cargo's output; building from Qt Creator
# for the emulator, set RUST_LIB_DIR the same way (see README.md).

TARGET = harbour-metascrub
CONFIG += sailfishapp
QT += quick

SOURCES += \
    src/main.cpp \
    src/scrubber.cpp

HEADERS += \
    src/scrubber.h

# The metascrub core, over its C ABI.
INCLUDEPATH += $$PWD/../../crates/metascrub-ffi/include
isEmpty(RUST_LIB_DIR): RUST_LIB_DIR = $$PWD/rustlib
LIBS += -L$$RUST_LIB_DIR -lmetascrub_ffi
# What the Rust staticlib (with libstd) needs at link time.
LIBS += -ldl -lpthread -lm

DISTFILES += \
    qml/harbour-metascrub.qml \
    qml/cover/CoverPage.qml \
    qml/pages/ScrubPage.qml \
    qml/pages/HandbookPage.qml \
    qml/pages/AboutPage.qml \
    qml/images/sandpiper.svg \
    qml/handbook.json \
    rpm/harbour-metascrub.spec \
    harbour-metascrub.desktop

# Deploy the whole QML tree (pages, cover, the sandpiper art, and the shared
# Handbook content that is kept in one place in the Android tree).
qml.files = qml
qml.path = /usr/share/$$TARGET
INSTALLS += qml

# A scalable SVG launcher icon, so no rasterization step is needed; Lipstick
# resolves it through the hicolor theme from the .desktop's Icon= name. The
# install target is deliberately NOT named `icon`: CONFIG += sailfishapp defines
# its own `icon` target (for rasterized PNGs), and a same-named target silently
# overrides ours, dropping the SVG from the package.
svgicon.files = icons/harbour-metascrub.svg
svgicon.path = /usr/share/icons/hicolor/scalable/apps
INSTALLS += svgicon
