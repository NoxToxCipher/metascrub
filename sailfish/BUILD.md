# Building metascrub for Sailfish OS

The SDK is installed and the native app scaffold is complete. This machine has one
hard constraint, and the build path below works around it entirely.

## The constraint: VBS vs VirtualBox

This machine runs Virtualization-Based Security (Memory Integrity / HVCI). VBS
owns the CPU's virtualization extensions, so VirtualBox falls back to the Windows
Hypervisor Platform (NEM) backend and the Sailfish VMs **hang at ~0.07 s of kernel
boot**. So the SDK's own VirtualBox build engine and the on-screen emulator cannot
run here while VBS is on (`VBox.log` shows "HM: Attempting fall back to NEM").

The build path below **does not use the VirtualBox engine at all** — it builds in
a Docker container — so it works with all security on. The emulator still needs
VirtualBox, so on-screen testing waits for a device or a less-locked machine.

## The build path (Docker, no VirtualBox, no SDK reconfigure)

Everything runs on the CLI against a working Docker engine (Docker Desktop with
the WSL2 backend runs fine alongside VBS). It does not touch the installed
`C:\SailfishOS` SDK.

### One-time: the platform-SDK image

```bash
docker pull coderus/sailfishos-platform-sdk:5.1.0.11
```

This ~20 GB image carries the 5.1.0.11 targets (aarch64 / armv7hl / i486), `mb2`,
and the Sailfish cross toolchains. The tag is version-matched to the installed
SDK's target.

### Step 1 — prebuild the Rust core (on the host)

The engine's Rust is 1.75, older than the workspace's edition-2024 dependencies
(e.g. `lopdf 0.44`) can be compiled by. So the core (`metascrub-ffi`) is
cross-compiled to a **static library** with the host's modern Rust. A staticlib is
just an archive of object files, so this needs no cross-linker — only the target
std. The final link against the Sailfish sysroot happens in step 2.

```bash
sailfish/build-ffi.sh aarch64      # or armv7 / emulator
# -> stages libmetascrub_ffi.a in sailfish/harbour-metascrub/rustlib/<cpu>/
```

`Cargo.lock` is pinned to lockfile **version 3** so the same lock is usable by the
older cargo, should you ever build the Rust inside the engine instead.

### Step 2 — build the RPM in the container

Mount the whole repo (the app's `.pro`/spec reach up into `crates/`) and run
`mb2` for the target. The spec's `%build` links the prebuilt `.a` from step 1; no
Rust runs in the engine.

```bash
docker run --rm --privileged \
  -v "C:/Users/lochr/metascrub:/home/mersdk/share" \
  coderus/sailfishos-platform-sdk:5.1.0.11 \
  bash -lc 'cd /home/mersdk/share/sailfish/harbour-metascrub \
            && sed -i "s/\r\$//" rpm/harbour-metascrub.spec \
            && mb2 -t SailfishOS-5.1.0.11-aarch64 build'
# -> sailfish/harbour-metascrub/RPMS/harbour-metascrub-0.1.0-1.aarch64.rpm
```

The `sed` strips CRLF from the spec (Windows checkout); `.gitattributes` keeps the
committed copies LF. Build output (`RPMS/`, `.mb2/`, `installroot/`, `*.o`, the
binary) is gitignored.

## State

- **aarch64 RPM builds.** ~800 KB, links the prebuilt Rust core, installs the
  Silica UI, QML and icon.
- **Not yet done (rpmlint polish, none blocking the build):** renormalize the
  working tree to LF and rebuild so the packaged QML/SVG are LF (rpmlint errors on
  CRLF); add a `%changelog`; provide rasterized PNG icons at the harbour sizes
  (the scalable SVG builds but harbour wants PNGs); strip the binary.
- **armv7hl / i486** targets are wired the same way; only aarch64 has been built.
- **On-screen test** needs the VirtualBox emulator (blocked by VBS) or a device.

## Installing on a device

```bash
# On a Sailfish device with Developer Mode on:
scp RPMS/harbour-metascrub-0.1.0-1.aarch64.rpm nemo@<device>:
# then, on the device:
devel-su pkcon install-local harbour-metascrub-0.1.0-1.aarch64.rpm
```
