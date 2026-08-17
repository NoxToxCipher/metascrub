# Reporting a security issue

Please report privately first, through a
[private security advisory](https://github.com/NoxToxCipher/metascrub/security/advisories/new)
on this repository, which only the maintainers can see.

Do not open a public issue for a security bug. People may be relying on this in
situations where the failure is not an inconvenience.

Expect an acknowledgement within **7 days**. If nothing arrives, assume the
message went astray and try again.

## What counts

Anything that makes the software protect someone less than its interface says it
does. Not exhaustive:

- Metadata surviving when the report says `Complete`.
- A file reported as cleaned that is corrupted, or that no longer opens.
- `Complete` reported when the container was not actually rebuilt.
- A panic, abort, or non-terminating loop reachable from a file. Denial of
  service counts: release builds set `panic = "abort"`, so a reachable panic
  ends the process.
- Memory exhaustion from a crafted file, such as a decompression bomb slipping
  past the size and megapixel limits.
- The application writing traces of which files were handled, beyond those
  documented below.
- Any network activity whatsoever. There is no networking code in either binary
  and there never should be.

## What does not count, and why

Documented limits rather than defects. Listed so nobody wastes time on them, and
so the remaining claims are checkable.

- **The original file still exists.** A cleaned copy is written alongside it.
  The interface says so.
- **Sensor fingerprints surviving `pixelwash`.** It reduces correlation. It does
  not remove the pattern, and nothing claims it does.
- **A platform recognising a washed image.** Perceptual hashes are designed to
  survive resizing and re-compression, which is exactly what `pixelwash` does.
  Different attack, different defence.
- **Anything about an upload being traceable.** The platform's own record of
  which account uploaded what is outside any file-cleaning tool's reach.
- **Recent-file entries the desktop keeps after you use the file picker.**
  Every desktop records them, in more places than one, and metascrub cleans up
  what it can reach immediately afterwards.

  On Windows the `Recent` folder shortcuts are removed;
  `AutomaticDestinations-ms` jump-list databases are an undocumented binary
  format and are not touched. On Linux, GTK's `recently-used.xbel` and KDE's
  `RecentDocuments` entries are removed; search indexes that keep their own
  copy, such as Tracker, Baloo, Zeitgeist and the KDE activity manager, are
  not. A desktop that has already read the list into a running process keeps
  its copy until it next writes one out.

  Drag and drop creates none of this on either platform, and is what the
  interface recommends.
- **Anything requiring an already-compromised operating system**, or physical
  access to an unlocked machine.

## Design decisions that are deliberate

Worth knowing before reporting them as bugs:

- **`#![forbid(unsafe_code)]`** in every crate. There is no unsafe code here at
  all.
- **No network dependencies.** Verifiable with `cargo tree`.
- **Panics do not produce crash dumps.** The interface installs a hook that
  exits directly, because at the moment of a crash the process holds a decoded
  photograph and whatever was extracted from it.
- **Output is written to a temporary file and renamed.** An interrupted write
  cannot leave a truncated file wearing a name that says it was cleaned.
- **A file that cannot be parsed produces no output at all.**

## Handling

Fixes land with a regression test, so the same bug cannot quietly return. Credit
is given unless you prefer otherwise.

There is no bounty. This is an unfunded project, and offering money it does not
have would be worse than saying so.
