# metascrub on ChromeOS

ChromeOS can run metascrub two ways, and they are not equally good. This
explains which one is supported, why, and what a Chromebook user has to do.

## The short version

Install the `.deb` into the Linux environment. It appears in the ChromeOS
launcher next to everything else, and nothing about it looks like a developer
tool once it is there.

```
sudo apt install ./metascrub-0.1.0-linux-amd64.deb
```

Use the `arm64` file instead on an ARM Chromebook. `uname -m` inside the Linux
terminal says which you have: `x86_64` or `aarch64`.

## Why the Linux environment and not the Android one

A Chromebook can also run the Android build, because ChromeOS runs Android
apps. That path is not the one being shipped, for a reason that has nothing to
do with which app is better.

metascrub is distributed by direct download, and on F-Droid, and by mirrors.
That is a deliberate decision: reach should never depend on surviving somebody's
review process, and a tool that is most useful to people in difficult places is
exactly the kind of tool that gets pulled from a store. On a Chromebook,
sideloading an Android app means turning on developer mode, which most people
will not do and should not have to. So the only realistic Android channel on
ChromeOS is the Play Store, which is the one channel the project has decided not
to depend on.

The Linux environment has no such gate. A `.deb` is a file you download and
install, the same as on any other computer.

**This is a decision worth revisiting, not a law.** Publishing the existing
Android build to the Play Store as an additional channel would cost no new code,
and would reach Chromebook users who will never enable the Linux environment. It
is listed as an open question in the launch checklist rather than settled here.

## Turning on the Linux environment

Settings, then **About ChromeOS**, then **Developers**, then **Linux
development environment**, then **Turn on**. It downloads a Debian container,
which takes a few minutes and about a gigabyte.

Despite the name, nothing about using it requires developing anything. It is
how any Linux application runs on a Chromebook.

## Reaching your files

This is the part that surprises people, and it is worth understanding rather
than working around.

The Linux environment has its own home directory, separate from the rest of the
Chromebook. It cannot see your Downloads folder, or Google Drive, or an SD card,
until you say so. That is a security boundary and it is a good one.

To let metascrub see a folder: open **Files**, right-click the folder, choose
**Share with Linux**. It then appears inside the Linux environment under
`/mnt/chromeos/`.

Two things follow from that:

- A cleaned copy saved back into a shared folder shows up in the ChromeOS Files
  app immediately, which is usually what you want.
- Anything in the Linux environment's own home directory is **not** visible to
  the Files app, and is not included in ChromeOS backups.

You can also drag a file straight from the Files app onto the metascrub window.
That works without sharing anything, and it is the path the application
recommends anyway, for a reason explained below.

## The file picker

Crostini is a deliberately minimal Debian. It does not include an XDG desktop
portal, which is the service a Linux application asks to show a file chooser.
Without it, the **Choose files** button has nothing to open.

If the `.deb` was installed with `apt` as shown above, this is already handled:
`xdg-desktop-portal-gtk` is a recommended dependency and apt installs it. If you
installed by double-clicking the file in the Files app, recommended dependencies
may be skipped, and metascrub will tell you so and name the package:

```
sudo apt install xdg-desktop-portal-gtk
```

Drag and drop works either way, and it is better for you regardless: the file
picker is the thing that writes your filename into the desktop's recent-files
list. metascrub cleans that list up afterwards, but not creating the entry at
all is stronger than removing it.

## What ChromeOS still knows

metascrub cleans files. It cannot clean the operating system around them, and on
a Chromebook that operating system is a Google product signed into a Google
account. Specifically:

- The **Files app** and ChromeOS itself keep their own record of recently opened
  files. metascrub's cleanup reaches the Linux environment's recent-files list,
  not the ChromeOS one on the other side of the boundary.
- If the folder you are working in is **Google Drive**, the original file is
  already uploaded, metadata and all. Cleaning a local copy does not reach it.
- ChromeOS backups and sync may already hold the original.

None of this is a reason not to use it. It is the same point the application
makes about uploads generally: cleaning a file handles what is inside the file.

## Known limits

- **Debian 11 or newer.** The binaries are built against glibc 2.31. Every
  current Crostini image is newer than that. A container from an old ChromeOS
  version that has never been recreated may not be; `ldd --version` in the
  Linux terminal will say.
- **The launcher icon can take a minute** to appear after installing, because
  ChromeOS scans the container's applications on a timer.
- **Not verified on hardware.** Everything here follows from how Crostini works
  and from the binaries being checked against the right glibc, but no
  maintainer has yet run it on a physical Chromebook. If you do, please say
  what happened, on either outcome.
