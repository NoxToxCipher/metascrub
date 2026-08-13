//! Small shared pieces: a bounds-checked byte reader and CRC-32.
//!
//! Both are hand-rolled rather than pulled in as dependencies. They are a few
//! dozen lines each, and every dependency here is one more thing that has to
//! cross-compile to the Android NDK and clear the licence gate.

/// A cursor over untrusted bytes. Every read is bounds-checked and returns an
/// `Option`, so a truncated or hostile file produces `None` and a clean parse
/// error rather than a panic.
///
/// This exists because the alternative, indexing slices directly, is one
/// forgotten length check away from a denial of service on a crate whose whole
/// job is handling files from strangers.
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    pub(crate) fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub(crate) fn seek(&mut self, pos: usize) -> Option<()> {
        (pos <= self.buf.len()).then(|| self.pos = pos)
    }

    /// The bytes between `start` and the cursor. Used to hand back a run that
    /// was consumed byte by byte, such as a JPEG entropy-coded scan.
    pub(crate) fn slice_from(&self, start: usize) -> &'a [u8] {
        self.buf.get(start..self.pos).unwrap_or_default()
    }

    pub(crate) fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let slice = self.buf.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    pub(crate) fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    pub(crate) fn u16_be(&mut self) -> Option<u16> {
        Some(u16::from_be_bytes(self.take(2)?.try_into().ok()?))
    }

    pub(crate) fn u32_be(&mut self) -> Option<u32> {
        Some(u32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }

    pub(crate) fn u64_be(&mut self) -> Option<u64> {
        Some(u64::from_be_bytes(self.take(8)?.try_into().ok()?))
    }

    pub(crate) fn u16_le(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }

    pub(crate) fn u32_le(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    /// Read a big- or little-endian `u16`, for TIFF where the byte order is
    /// declared in the file.
    pub(crate) fn u16_endian(&mut self, big: bool) -> Option<u16> {
        if big {
            self.u16_be()
        } else {
            self.u16_le()
        }
    }

    /// Read a big- or little-endian `u32`.
    pub(crate) fn u32_endian(&mut self, big: bool) -> Option<u32> {
        if big {
            self.u32_be()
        } else {
            self.u32_le()
        }
    }
}

/// CRC-32 (IEEE 802.3, reflected, `0xEDB88320`) lookup table, built at compile
/// time. A `const` block means no runtime init and no dependency — the reasons
/// the bitwise form was originally chosen — while giving the table's ~4-8x
/// speed. For a large PNG (`IDAT` is verified byte-for-byte, up to the 2 GB
/// ceiling) or a rewritten ZIP entry, the CRC dominates the sanitize cost. ~1 KiB.
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n = 0usize;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
};

/// Fold `data` into a running (pre-final-XOR) CRC. Identical result to the
/// bitwise form; validated by `crc32_matches_known_vectors`.
#[inline]
fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    for &byte in data {
        crc = CRC32_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc
}

/// CRC-32 as used by PNG and ZIP.
// PNG hashes chunks through `crc32_parts`; the whole-buffer form is for ZIP
// entries, so an image-only build does not reach it.
#[cfg_attr(not(feature = "ooxml"), allow(dead_code))]
pub(crate) fn crc32(data: &[u8]) -> u32 {
    !crc32_update(0xFFFF_FFFF, data)
}

/// CRC-32 over several slices, so a PNG chunk's type and body can be hashed
/// without first joining them into one allocation.
pub(crate) fn crc32_parts(parts: &[&[u8]]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for part in parts {
        crc = crc32_update(crc, part);
    }
    !crc
}

/// Case-insensitive ASCII `starts_with`, for magic strings whose case varies
/// between producers (`Exif\0\0`, `ICC_PROFILE\0`).
pub(crate) fn starts_with_ignore_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.len() >= needle.len() && haystack[..needle.len()].eq_ignore_ascii_case(needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_known_vectors() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        // The PNG IEND chunk's CRC is a fixed, widely published value.
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn crc32_parts_matches_the_joined_form() {
        assert_eq!(crc32_parts(&[b"123", b"456", b"789"]), crc32(b"123456789"));
        assert_eq!(crc32_parts(&[]), crc32(b""));
        assert_eq!(crc32_parts(&[b"", b"a", b""]), crc32(b"a"));
    }

    #[test]
    fn reader_refuses_to_run_past_the_end() {
        let mut r = Reader::new(&[1, 2, 3]);
        assert_eq!(r.u16_be(), Some(0x0102));
        assert_eq!(r.remaining(), 1);
        assert_eq!(r.u16_be(), None, "a short read must not be a partial read");
        assert_eq!(r.pos(), 2, "a failed read must not move the cursor");
        assert_eq!(r.u8(), Some(3));
        assert!(r.is_empty());
    }

    #[test]
    fn reader_survives_an_overflowing_length() {
        let mut r = Reader::new(&[1, 2, 3]);
        assert_eq!(r.take(usize::MAX), None);
        assert_eq!(r.seek(9), None);
    }

    #[test]
    fn endian_helpers_agree_with_the_fixed_ones() {
        assert_eq!(Reader::new(&[0x12, 0x34]).u16_endian(true), Some(0x1234));
        assert_eq!(Reader::new(&[0x12, 0x34]).u16_endian(false), Some(0x3412));
        assert_eq!(Reader::new(&[1, 2, 3, 4]).u32_endian(true), Some(0x0102_0304));
        assert_eq!(Reader::new(&[1, 2, 3, 4]).u32_endian(false), Some(0x0403_0201));
    }
}
