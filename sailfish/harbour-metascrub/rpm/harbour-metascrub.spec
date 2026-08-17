Name:       harbour-metascrub
Summary:    Remove metadata from your files, on your device
Version:    0.1.0
Release:    1
License:    GPLv3+
URL:        https://github.com/NoxToxCipher/metascrub
Source0:    %{name}-%{version}.tar.bz2

Requires:   sailfishsilica-qt5 >= 0.10.9
BuildRequires:  pkgconfig(sailfishapp) >= 1.0.2
BuildRequires:  pkgconfig(Qt5Core)
BuildRequires:  pkgconfig(Qt5Qml)
BuildRequires:  pkgconfig(Qt5Quick)
BuildRequires:  desktop-file-utils
# No Rust toolchain is required in the build engine: the Rust core is prebuilt as
# a static library on a modern-Rust host (the engine's Rust is too old for the
# workspace's edition-2024 deps) and only linked here. See the build section below
# and BUILD.md.

%description
metascrub removes the hidden data a file carries about you: where a photo was
taken, which camera and account made a document, the editing history. It tells
you honestly how much it could remove. Everything happens on the device.
Nothing is uploaded, and the app asks for no network access.

%prep
%setup -q -n %{name}-%{version}

%build
# 1) Locate the prebuilt metascrub core (crate metascrub-ffi) as a C-ABI static
#    library for this arch. It is built OUTSIDE the Sailfish build engine, on a
#    modern-Rust host, because the engine's Rust (1.75 in 5.1.0.11) is older than
#    the workspace's edition-2024 dependencies (e.g. lopdf 0.44) can be compiled
#    by. A staticlib needs no linker to produce, so that cross-build is clean; the
#    final link against the Sailfish sysroot happens in step 2. Build it first
#    with sailfish/build-ffi.sh %{_target_cpu} (see BUILD.md).
#
#    %{_sourcedir} is the project's rpm/ dir under mb2, so the libs live one level
#    up in rustlib/<target-cpu>/.
RUST_LIB_DIR=%{_sourcedir}/../rustlib/%{_target_cpu}
if [ ! -f "$RUST_LIB_DIR/libmetascrub_ffi.a" ]; then
    echo "error: prebuilt libmetascrub_ffi.a for %{_target_cpu} not found in $RUST_LIB_DIR" >&2
    echo "build it first on a modern-Rust host: sailfish/build-ffi.sh %{_target_cpu}" >&2
    exit 1
fi

# 2) Build the Silica app, linking the static library from step 1.
%qmake5 RUST_LIB_DIR="$RUST_LIB_DIR"
%make_build

%install
%qmake5_install

desktop-file-install --delete-original \
  --dir %{buildroot}%{_datadir}/applications \
  %{buildroot}%{_datadir}/applications/*.desktop

# QML, JSON and SVG are data, not scripts. qmake's install can carry an executable
# bit across from a Windows checkout, which makes rpmlint flag every one as a
# script-without-shebang. Force them to plain 0644.
find %{buildroot}%{_datadir}/%{name} -type f -exec chmod 0644 {} +
find %{buildroot}%{_datadir}/icons -type f -exec chmod 0644 {} +

%files
%defattr(-,root,root,-)
%{_bindir}/%{name}
%{_datadir}/%{name}
%{_datadir}/applications/%{name}.desktop
%{_datadir}/icons/hicolor/86x86/apps/%{name}.png
%{_datadir}/icons/hicolor/108x108/apps/%{name}.png
%{_datadir}/icons/hicolor/128x128/apps/%{name}.png
%{_datadir}/icons/hicolor/172x172/apps/%{name}.png

%changelog
* Mon Aug 17 2026 NoxToxCipher <github.elitism514@passmail.com> - 0.1.0-1
- First Sailfish package: native Silica UI over the pure-Rust metascrub core.
