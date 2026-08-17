# Launch checklist: Linux and ChromeOS

What is ready, what is not, and what nobody can tick off from a keyboard.

## Ready

- [x] **The binaries start on other people's machines.** Built against glibc
      2.31 in a Debian bullseye container, which covers every supported
      distribution and both current Crostini images. The Ubuntu 24.04 build
      wanted 2.39 and would not have started on Debian 12, Ubuntu 22.04,
      RHEL 9, Mint 21 or any Chromebook.
- [x] **The command line tool is static.** musl, 1.26 MB, no glibc floor at
      all. Runs on Alpine and on rescue images.
- [x] **The release gate works.** It used to fail a clean container build
      because the account was called `root` and the binary contains the word
      "chroot". It now checks for path prefixes as fixed strings, prints what
      it found, and hashes whatever the host actually built instead of naming
      two `.exe` files and swallowing the error.
- [x] **Desktop integration.** Desktop entry, eight icon sizes plus a
      scalable one, AppStream metadata, file associations, `%F` handling so
      "Open with" works, `--help` and `--version`.
- [x] **The window has an icon** even when nothing has been installed, drawn
      from the same shapes as the launcher icon.
- [x] **The session language is picked up** from `$LANG` instead of opening in
      English and offering a two-letter toggle to somebody who cannot read it.
- [x] **The Linux recent-files leak is closed.** GTK's `recently-used.xbel`
      and KDE's `RecentDocuments`, on the same terms as the Windows cleanup.
- [x] **No dead file-picker button.** Crostini has no XDG portal, so the
      picker did nothing and said nothing. It now explains and names the
      package.
- [x] **Packages.** Tarball, `.deb`, AppImage staging directory, all
      reproducible from a commit.
- [x] **CI builds them** for amd64 and arm64 on a tag, and validates the
      desktop entry, the AppStream file, the four places carrying the
      application ID, and the committed icons against their generator.
- [x] **LICENSE.** The project has declared GPL-3.0-or-later since the first
      commit and never carried the text.

## Before tagging

- [ ] **Set `DEB_MAINTAINER`** to a real contact address. The default is a
      GitHub noreply placeholder: valid, and says nothing.
- [ ] **Sign the releases.** The hashes are reproducible, which is necessary
      and not sufficient. This is the last item standing between "working" and
      "released", and it is the same gap the Windows and Android builds have,
      so solve it once for all three.
- [ ] **Take a screenshot** for `org.crake.metascrub.metainfo.xml`. Flathub
      requires at least one and it has to come from a real running window.
      Deliberately absent rather than faked. Not needed for the tarball or the
      `.deb`.
- [ ] **Run it on a real Chromebook.** Everything in `docs/chromeos.md`
      follows from how Crostini works and from the binaries being checked
      against the right glibc, but no maintainer has run it on hardware. The
      document says so, and that sentence should either become false or stay.
- [ ] **Run it on one GNOME and one KDE machine.** Specifically: does the
      launcher icon appear, does "Open with" hand the file over, and does the
      recent-files cleanup actually catch the entry the portal writes. The
      cleanup logic is tested; what the portal writes on a given desktop is
      not something a unit test can tell you.

## Open decisions

- **The Play Store, for ChromeOS.** ChromeOS is being shipped through Crostini
  and the `.deb`, because sideloading an Android app on a Chromebook needs
  developer mode and the only realistic Android channel there is the Play
  Store. Publishing the existing APK to the Play Store as an *additional*
  channel would cost no new code and would reach Chromebook users who will
  never enable the Linux environment. It conflicts with nothing as long as it
  is never the only channel. Worth deciding deliberately rather than by
  default.
- **Flathub.** Would reach Fedora, Silverblue, SteamOS and most immutable
  distributions, where a `.deb` is useless and a tarball is awkward. Costs a
  screenshot, a submission and a review cycle. Not a launch blocker; the
  tarball covers those users meanwhile.

## Not blocking, worth knowing

- **The desktop ships four languages; Android ships eleven.** That gap is
  desktop-against-Android, not Linux-against-Windows: both desktop platforms
  are the same crate and ship the same four, so Linux launches at parity with
  Windows.

  Four of the seven missing languages are Latin or Cyrillic and could be
  added whenever somebody can review them: Belarusian, Esperanto, Ukrainian
  and Kurmanji. The other three cannot be added yet at any quality. Arabic,
  Farsi and Sorani are right-to-left, and egui has no bidirectional text
  shaping, so the strings would render as disconnected letters in the wrong
  order. That looks broken rather than foreign, and on a screen whose job is
  to say COMPLETE or NOT CLEANED, unreadable is worse than English.

- **A core dump can still be written if a graphics driver crashes.** Rust
  panics are covered, and this crate has no `unsafe`, but a segmentation fault
  inside the system OpenGL libraries is not. Recorded as a residual risk in
  the threat model, because closing it needs an exception to
  `#![forbid(unsafe_code)]` and that is a decision, not a chore.

- **arm64 is built but unverified.** CI produces it by running the same
  container under emulation. Nobody has run the result on an ARM machine.
