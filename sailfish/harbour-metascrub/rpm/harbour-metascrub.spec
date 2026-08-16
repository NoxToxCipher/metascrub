Name:       harbour-metascrub
Summary:    Remove metadata from your files, on your device
Version:    0.1.0
Release:    1
License:    GPL-3.0-or-later
URL:        https://github.com/NoxToxCipher/metascrub
Source0:    %{name}-%{version}.tar.bz2

Requires:   sailfishsilica-qt5 >= 0.10.9
BuildRequires:  pkgconfig(sailfishapp) >= 1.0.2
BuildRequires:  pkgconfig(Qt5Core)
BuildRequires:  pkgconfig(Qt5Qml)
BuildRequires:  pkgconfig(Qt5Quick)
BuildRequires:  desktop-file-utils
# The core is Rust; the Sailfish build environment needs a Rust toolchain with
# the target for this arch installed (rustup target add ...). See README.md.
BuildRequires:  rust
BuildRequires:  cargo

%description
metascrub removes the hidden data a file carries about you — where a photo was
taken, which camera and account made a document, the editing history — and tells
you honestly how much it could remove. Everything happens on the device. Nothing
is uploaded, and the app asks for no network access.

%prep
%setup -q -n %{name}-%{version}

%build
# 1) Build the metascrub core as a C-ABI static library for this arch.
#    The Rust target is chosen from the RPM target CPU.
case "%{_target_cpu}" in
    armv7hl)  RUST_TARGET=armv7-unknown-linux-gnueabihf ;;
    aarch64)  RUST_TARGET=aarch64-unknown-linux-gnu ;;
    i486|i686) RUST_TARGET=i686-unknown-linux-gnu ;;
    *) echo "unmapped target cpu %{_target_cpu}" >&2; exit 1 ;;
esac
# The crates live in the workspace above this app. RUST_WORKSPACE defaults to the
# repository root two levels up (adjust when packaging a standalone tarball).
RUST_WORKSPACE=%{_sourcedir}/../..
cargo build --release --manifest-path "$RUST_WORKSPACE/Cargo.toml" \
    -p metascrub-ffi --target "$RUST_TARGET"
RUST_LIB_DIR="$RUST_WORKSPACE/target/$RUST_TARGET/release"

# 2) Build the Silica app, linking the static library from step 1.
%qmake5 RUST_LIB_DIR="$RUST_LIB_DIR"
%make_build

%install
%qmake5_install

desktop-file-install --delete-original \
  --dir %{buildroot}%{_datadir}/applications \
  %{buildroot}%{_datadir}/applications/*.desktop

%files
%defattr(-,root,root,-)
%{_bindir}/%{name}
%{_datadir}/%{name}
%{_datadir}/applications/%{name}.desktop
%{_datadir}/icons/hicolor/scalable/apps/%{name}.svg
