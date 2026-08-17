# Building the Linux release

Two scripts. The first produces binaries that start on other people's machines,
the second turns them into downloads.

```sh
packaging/linux/build-in-container.sh --arch amd64
packaging/linux/package.sh            --arch amd64
```

Output lands in `dist/`. Repeat with `--arch arm64` for ARM machines, including
ARM Chromebooks, which are a large share of Chromebooks.

## Why a container

A Linux binary demands the glibc version of whatever machine compiled it, and
refuses to start on anything older. Compiled on Ubuntu 24.04, the GUI came out
requiring `GLIBC_2.39`, which rules out Debian 12, Ubuntu 22.04, RHEL 9, Linux
Mint 21 and both current ChromeOS Crostini images. The loader gives up before
`main()` runs, so the application cannot detect it, warn about it, or degrade;
the user sees a version error in a terminal nobody told them to open.

Nothing in this repository causes it and no compiler flag fixes it. The only
answer is to compile somewhere older, so `build-in-container.sh` uses Debian
bullseye and glibc 2.31. `build-desktop.sh` refuses any Linux build above that
floor for the same reason.

The command line tool is built for musl instead, statically, because it has no C
dependencies and so it costs nothing. It ends up with no glibc floor at all: one
file that runs on anything with a Linux kernel, Alpine and rescue images
included. That does not extend to the GUI, which reaches the system OpenGL and
Wayland libraries through `dlopen`, and those are glibc builds that will not
load into a musl process.

## Why there are no `-dev` packages anywhere

There is nothing to link against. X11, Wayland, EGL, xkbcommon and dbus are all
opened at runtime, and `rfd` reaches the file chooser through the XDG portal
rather than GTK, so there is no `gtk-sys` in the lockfile. The container
installs a compiler, `curl` and nothing else.

This is worth protecting. It is why one binary works on GNOME, KDE, Xfce, a
tiling window manager and Crostini without a per-distribution build, and it is
easy to lose by adding one dependency that wants a `.pc` file.

## What each format is for

| File | For |
|---|---|
| `.tar.gz` | Every distribution. Unpack, run `install.sh`, no root needed. Depends on nothing but a shell, so it is the one that has to keep working. |
| `.deb` | Debian, Ubuntu, Mint, and the Debian container inside ChromeOS. This is the whole ChromeOS story; see `docs/chromeos.md`. |
| `.AppDir` | Staging for an AppImage. `package.sh` finishes the job if `appimagetool` is on the machine, and leaves the directory if not, rather than downloading a tool behind your back. |

## Reproducibility

Both archives are built with fixed timestamps and ownership, and the gzip header
is written without a name or a time. Given the same commit they produce the same
bytes, which is the only way a hash on a download page means anything. CI passes
`SOURCE_DATE_EPOCH` from the commit rather than the clock.

The binaries themselves are remapped so they do not carry the path of the
machine that built them. That check runs in `build-in-container.sh` and again in
`build-desktop.sh`, because a container has a home directory too.

## Before tagging a release

- [ ] `DEB_MAINTAINER` set to a real contact address. The default is a GitHub
      noreply placeholder, which is valid but says nothing.
- [ ] A screenshot in `org.crake.metascrub.metainfo.xml`. Flathub requires at
      least one and it has to come from a real running window, so it is
      deliberately absent rather than faked.
- [ ] Signatures. Nothing here is signed yet. The hashes are reproducible,
      which is necessary and not sufficient.
- [ ] `docs/chromeos.md` says the ChromeOS path has not been run on physical
      hardware. Either run it or leave that sentence in place.

## Regenerating the icons

```sh
python3 packaging/linux/make-icons.py
```

Deterministic, and CI checks the committed files against its output. The
generator draws the same shapes the application draws in its own window
(`crates/metascrub-gui/src/icon.rs`) from the same coordinates, so the launcher
icon and the mark inside the window cannot drift apart without somebody
noticing.
