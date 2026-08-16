# Building metascrub for Sailfish OS — real state on this machine

The SDK is installed and the native app scaffold is complete. The remaining work
is the **build environment**, and this machine has one hard constraint that
shapes everything below.

## What's installed and working
- **Sailfish SDK 3.13.5** at `C:\SailfishOS` (note: root of C:, not under the user
  folder). `sfdk` at `C:\SailfishOS\bin\sfdk.exe`.
- **VirtualBox 7.2.14**, with two VMs registered: `Sailfish SDK Build Engine`
  and the `SailfishOS-5.1.0.11` emulator.
- Rust cross-targets installed on the host: `i686-unknown-linux-gnu` (emulator),
  `armv7-unknown-linux-gnueabihf`, `aarch64-unknown-linux-gnu`.

## The hard constraint: VBS vs VirtualBox
This machine runs **Virtualization-Based Security** — Memory Integrity (HVCI),
System Guard, SMM measurement — all on. VBS owns the CPU's AMD-V, so VirtualBox
falls back to the Windows Hypervisor Platform (NEM) backend, and the Sailfish VMs
**hang at ~0.07 s of kernel boot** (confirmed: `VBox.log` shows "HM: Attempting
fall back to NEM: AMD-V is not available"; the build-engine console freezes right
after the Spectre line). SSH to the build engine on port 2222 connects but never
gets a banner — the guest never finishes booting.

**Net:** the VirtualBox build engine and the emulator cannot boot here while VBS
is on. Two ways forward, one per priority:

### Path A — run the emulator (weakens security, reversible)
Only if you want the on-screen emulator test on *this* machine.
1. Windows Security → Device security → Core isolation → **Memory integrity: Off**.
2. If VBS is still on after reboot (System Guard can hold it), also turn off
   **Firmware protection**, and as a backstop set (admin):
   `bcdedit /set hypervisorlaunchtype off`.
3. Reboot. Confirm off: `Get-CimInstance -Namespace root\Microsoft\Windows\DeviceGuard -ClassName Win32_DeviceGuard` → `VirtualizationBasedSecurityStatus` should be 0.
4. `sfdk engine start` → should connect. Then Path C below.
5. Re-enable Memory Integrity afterwards.

### Path B — keep all security on, build only (no emulator here)
The build-engine backend is **locked at install** (`sfdk` can't switch it), so:
1. Install **Docker Desktop** (WSL2 backend — runs fine alongside VBS).
2. Reconfigure the SDK's build engine to Docker: re-run
   `C:\SailfishOS\SDKMaintenanceTool.exe` (or the SDK installer) and choose the
   **Docker** build engine. (VirtualBox and Docker engines can't coexist for one
   SDK, and the type can't be flipped in place.)
3. `sfdk engine start` (Docker engine) → then Path C.
4. The **emulator still needs VirtualBox**, so on-screen testing waits for a
   less-locked-down machine or a real device.

## Path C — the actual build (after A or B gives a working engine)
The one genuinely novel step: the **Rust cross-compile inside the build engine**.
1. Make sure Rust is available in the engine, or cross with the target sysroot.
   `sailfish/build-ffi.sh emulator` drives the cargo build for `i686`.
2. `sfdk config target=SailfishOS-5.1.0.11-i486`
3. `sfdk build sailfish/harbour-metascrub` (the RPM `%build` runs the cargo step,
   then links `libmetascrub_ffi` into the Silica app).
4. Path A only: `sfdk deploy --sdk` to install the RPM on the running emulator.

## Recommendation
Path B is the security-preserving one, but it's a fresh setup session (Docker
install + SDK reconfigure) on top of the Rust cross-compile. Do it focused, not
at the tail of a long night. Everything above it — SDK, scaffold, diagnosis — is
done and waiting.
