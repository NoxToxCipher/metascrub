# Flatten every timestamp in a ZIP, in place, without moving a byte.
#
#   python zip-time.py app.apk           # patch
#   python zip-time.py --check app.apk   # report, change nothing
#
# ZIP stores modification times as MS-DOS date/time, which has no timezone
# field: the writer stores its own LOCAL clock. So an APK does not merely say
# when it was built, it says when it was built *where the builder was*. Compare
# that against any UTC reference -- the release announcement, a commit, the PE
# timestamp of the desktop binary from the same session -- and the difference
# is the build machine's UTC offset. For a project whose author is deliberately
# pseudonymous, that is a location narrowed for free by a field nobody looks at.
#
# It is also the last thing keeping the APKs from being byte-reproducible: two
# people building identical source at different moments get different archives.
#
# The fields are patched where they sit rather than by rewriting the archive,
# because the .so is stored uncompressed and page-aligned by `zipalign -P 16`,
# and repacking would destroy that alignment. Same length in, same length out.
#
# Must run BEFORE apksigner: the v2/v3 signature covers these bytes.
#
# --check exists for archives this script cannot patch: bundletool writes the
# split APKs inside a .apks set and signs them itself, so their timestamps are
# whatever it chose. Checking says plainly whether a clock leaked instead of
# leaving it assumed either way.
import sys, struct

# 1980-01-01 00:00:00 -- the zero of the DOS epoch, and the only value that
# carries no information at all about when or where the build happened.
DOS_TIME = 0
DOS_DATE = (0 << 9) | (1 << 5) | 1

# The DOS fields are not the only clock in a ZIP, which is easy to miss because
# nothing displays them: Info-ZIP writes extra field 0x5455 (Unix seconds) and
# Windows tools write 0x000A (NTFS FILETIMEs), and `unzip -l` shows those in
# preference to the DOS date. Flattening only the DOS fields therefore produces
# an archive that reads as clean and still carries the build time to the second.
# `zip -X` leaves them out; anything already written is zeroed here.
EXTRA_UNIX_TIME = 0x5455
EXTRA_NTFS_TIME = 0x000A


def entries(d, path):
    """Walk the central directory.

    Yields one record per entry: the name, the offsets of its two DOS time
    fields (central and local), and the spans of its two extra fields.
    """
    # End of central directory: scan back, since a trailing comment may follow.
    eocd = d.rfind(b"PK\x05\x06")
    if eocd < 0:
        raise SystemExit(f"{path}: no end-of-central-directory record")
    total = struct.unpack_from("<H", d, eocd + 10)[0]
    p = struct.unpack_from("<I", d, eocd + 16)[0]

    for n in range(total):
        if d[p:p + 4] != b"PK\x01\x02":
            raise SystemExit(f"{path}: central directory entry {n} not where it should be")

        name_len  = struct.unpack_from("<H", d, p + 28)[0]
        extra_len = struct.unpack_from("<H", d, p + 30)[0]
        cmt_len   = struct.unpack_from("<H", d, p + 32)[0]
        lho       = struct.unpack_from("<I", d, p + 42)[0]
        name      = bytes(d[p + 46:p + 46 + name_len]).decode("utf-8", "replace")

        # The local header carries its own copy of both, and its own lengths:
        # the local extra field is often longer than the central one.
        if d[lho:lho + 4] != b"PK\x03\x04":
            raise SystemExit(f"{path}: local header for entry {n} not at {lho}")
        l_name_len  = struct.unpack_from("<H", d, lho + 26)[0]
        l_extra_len = struct.unpack_from("<H", d, lho + 28)[0]

        yield (name,
               (p + 12, lho + 10),
               ((p + 46 + name_len, extra_len),
                (lho + 30 + l_name_len, l_extra_len)))

        p += 46 + name_len + extra_len + cmt_len


def flatten_extra(d, off, length, dry_run):
    """Zero the clocks in an extra field. Returns how many were not already zero."""
    changed = 0
    p, end = off, off + length
    while p + 4 <= end:
        field_id, size = struct.unpack_from("<HH", d, p)
        body, nxt = p + 4, p + 4 + size
        if nxt > end:
            break  # malformed; leave the rest alone rather than guess

        if field_id == EXTRA_UNIX_TIME:
            # A flags byte, then one 32-bit time per flag set (mtime, atime, ctime).
            for i in range(body + 1, min(nxt, body + 13), 4):
                if i + 4 <= nxt and struct.unpack_from("<I", d, i)[0] != 0:
                    changed += 1
                    if not dry_run:
                        struct.pack_into("<I", d, i, 0)
        elif field_id == EXTRA_NTFS_TIME:
            # Reserved(4), then tagged attributes; tag 1 holds three FILETIMEs.
            q = body + 4
            while q + 4 <= nxt:
                tag, tag_size = struct.unpack_from("<HH", d, q)
                if tag == 1:
                    for i in range(q + 4, min(q + 4 + tag_size, nxt), 8):
                        if i + 8 <= nxt and struct.unpack_from("<Q", d, i)[0] != 0:
                            changed += 1
                            if not dry_run:
                                struct.pack_into("<Q", d, i, 0)
                q += 4 + tag_size

        p = nxt
    return changed


def flatten(d, record, dry_run=False):
    """Flatten one entry's clocks. Returns how many fields still carried one."""
    _name, dos_offsets, extra_spans = record
    changed = 0
    for off in dos_offsets:
        if struct.unpack_from("<HH", d, off) != (DOS_TIME, DOS_DATE):
            changed += 1
            if not dry_run:
                struct.pack_into("<HH", d, off, DOS_TIME, DOS_DATE)
    for off, length in extra_spans:
        changed += flatten_extra(d, off, length, dry_run)
    return changed


def patch(path):
    d = bytearray(open(path, "rb").read())
    count = 0
    for record in entries(d, path):
        flatten(d, record)
        count += 1
    open(path, "wb").write(d)
    return count


def check(path):
    """Names of entries whose timestamps still record when the build ran."""
    d = bytearray(open(path, "rb").read())
    return [record[0] for record in entries(d, path) if flatten(d, record, dry_run=True)]


if __name__ == "__main__":
    args = sys.argv[1:]
    if args[:1] == ["--check"]:
        if len(args) != 2:
            raise SystemExit("usage: zip-time.py --check <archive>")
        stamped = check(args[1])
        if stamped:
            shown = ", ".join(stamped[:5]) + (", ..." if len(stamped) > 5 else "")
            print(f"   {len(stamped)} entries carry a build timestamp: {shown}")
            raise SystemExit(1)
        print("   no entry carries a build timestamp")
    else:
        if len(args) != 1:
            raise SystemExit("usage: zip-time.py [--check] <archive>")
        print(f"   {patch(args[0])} entries flattened to 1980-01-01 00:00:00")
