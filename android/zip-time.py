# Flatten every timestamp in a ZIP, in place, without moving a byte.
#
#   python zip-time.py app.apk
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
# because the .so is stored uncompressed and page-aligned by `zipalign -p`, and
# repacking would destroy that alignment. Same length in, same length out.
#
# Must run BEFORE apksigner: the v2/v3 signature covers these bytes.
import sys, struct

# 1980-01-01 00:00:00 -- the zero of the DOS epoch, and the only value that
# carries no information at all about when or where the build happened.
DOS_TIME = 0
DOS_DATE = (0 << 9) | (1 << 5) | 1


def patch(path):
    d = bytearray(open(path, "rb").read())

    # End of central directory: scan back, since a trailing comment may follow.
    eocd = d.rfind(b"PK\x05\x06")
    if eocd < 0:
        raise SystemExit(f"{path}: no end-of-central-directory record")
    total = struct.unpack_from("<H", d, eocd + 10)[0]
    cd_off = struct.unpack_from("<I", d, eocd + 16)[0]

    n = 0
    p = cd_off
    for _ in range(total):
        if d[p:p + 4] != b"PK\x01\x02":
            raise SystemExit(f"{path}: central directory entry {n} not where it should be")
        struct.pack_into("<HH", d, p + 12, DOS_TIME, DOS_DATE)

        name_len  = struct.unpack_from("<H", d, p + 28)[0]
        extra_len = struct.unpack_from("<H", d, p + 30)[0]
        cmt_len   = struct.unpack_from("<H", d, p + 32)[0]
        lho       = struct.unpack_from("<I", d, p + 42)[0]

        # The local header carries its own copy of the same two fields.
        if d[lho:lho + 4] != b"PK\x03\x04":
            raise SystemExit(f"{path}: local header for entry {n} not at {lho}")
        struct.pack_into("<HH", d, lho + 10, DOS_TIME, DOS_DATE)

        p += 46 + name_len + extra_len + cmt_len
        n += 1

    open(path, "wb").write(d)
    return n


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: zip-time.py <archive>")
    count = patch(sys.argv[1])
    print(f"   {count} entries set to 1980-01-01 00:00:00")
