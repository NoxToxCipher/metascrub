//! Deterministic mutation testing for the container parsers.
//!
//! `cargo-fuzz` is the right tool for this and lives in `fuzz/`, but it needs a
//! nightly toolchain and libFuzzer, so it does not run on every developer's
//! machine or on every CI target. This is the portable floor: take valid files,
//! corrupt them in the ways a real damaged or hostile file is corrupted, and
//! require that the parser returns rather than panicking.
//!
//! The bar is deliberately low and absolute. A malformed file may legitimately
//! produce an error, an empty report, or a smaller output. What it may never do
//! is panic, hang, or claim `Complete` on something it did not fully rebuild.
//! `panic = "abort"` is set for release builds, so a reachable panic is a
//! process kill, and this crate's whole job is reading files from strangers.
//!
//! The seed is fixed, so a failure here reproduces exactly.

use metascrub::{sanitize, Assurance, Policy};

/// SplitMix64. Deterministic, so any failure is reproducible from the printed
/// iteration number.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

fn jpeg_seed() -> Vec<u8> {
    let seg = |m: u8, p: &[u8]| {
        let mut v = vec![0xFF, m];
        v.extend_from_slice(&((p.len() + 2) as u16).to_be_bytes());
        v.extend_from_slice(p);
        v
    };
    let mut j = vec![0xFF, 0xD8];
    j.extend(seg(0xE0, b"JFIF\0\x01\x02\x01\0\x48\0\x48\0\0"));
    j.extend(seg(0xE1, b"Exif\0\0MM\0\x2a\0\0\0\x08\0\0"));
    j.extend(seg(0xDB, &[0u8; 65]));
    j.extend(seg(0xC0, &[8, 0, 8, 0, 8, 1, 1, 0x11, 0]));
    j.extend(seg(0xC4, &[0u8; 20]));
    j.extend(seg(0xDA, &[1, 1, 0, 0, 63, 0]));
    j.extend_from_slice(&[0x12, 0x34, 0xFF, 0x00, 0x56]);
    j.extend_from_slice(&[0xFF, 0xD9]);
    j
}

fn png_seed() -> Vec<u8> {
    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                let mask = (c & 1).wrapping_neg();
                c = (c >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !c
    }
    fn chunk(ty: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = (body.len() as u32).to_be_bytes().to_vec();
        v.extend_from_slice(ty);
        v.extend_from_slice(body);
        let mut hashed = ty.to_vec();
        hashed.extend_from_slice(body);
        v.extend_from_slice(&crc(&hashed).to_be_bytes());
        v
    }
    let mut p = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    p.extend(chunk(b"IHDR", &ihdr));
    p.extend(chunk(b"tEXt", b"Author\0Someone"));
    p.extend(chunk(b"IDAT", &[0x78, 0x9C, 0x63, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01]));
    p.extend(chunk(b"IEND", b""));
    p
}

fn webp_seed() -> Vec<u8> {
    let mut body = b"WEBP".to_vec();
    body.extend_from_slice(b"VP8 ");
    body.extend_from_slice(&10u32.to_le_bytes());
    body.extend_from_slice(&[0u8; 10]);
    let mut w = b"RIFF".to_vec();
    w.extend_from_slice(&(body.len() as u32).to_le_bytes());
    w.extend_from_slice(&body);
    w
}

fn seeds() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("jpeg", jpeg_seed()),
        ("png", png_seed()),
        ("webp", webp_seed()),
        ("pdflike", b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n%%EOF".to_vec()),
        ("ziplike", b"PK\x03\x04\x14\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0".to_vec()),
    ]
}

/// Corrupt `data` the way real damage and real attacks do: flip a byte, cut it
/// short, splice in a huge length, repeat a region.
fn mutate(rng: &mut Rng, data: &mut Vec<u8>) {
    if data.is_empty() {
        return;
    }
    match rng.below(6) {
        0 => {
            let i = rng.below(data.len());
            data[i] = rng.next() as u8;
        }
        1 => {
            // Length fields are the interesting target: an absurd length is how
            // a parser gets talked into reading past the end of the buffer.
            let i = rng.below(data.len());
            for b in data.iter_mut().skip(i).take(4) {
                *b = 0xFF;
            }
        }
        2 => {
            let cut = rng.below(data.len());
            data.truncate(cut);
        }
        3 => {
            let i = rng.below(data.len());
            let byte = rng.next() as u8;
            data.insert(i, byte);
        }
        4 => {
            // 0xFF runs drive the JPEG marker scanner.
            let i = rng.below(data.len());
            for b in data.iter_mut().skip(i).take(3) {
                *b = 0xFF;
            }
        }
        _ => {
            let i = rng.below(data.len());
            let n = rng.below(32).min(data.len() - i);
            let slice: Vec<u8> = data[i..i + n].to_vec();
            data.extend_from_slice(&slice);
        }
    }
}

#[test]
fn mutated_files_never_panic() {
    let policy = Policy::default();
    let mut rng = Rng(0x5EED_1234_ABCD_0001);

    for iteration in 0..20_000u32 {
        let (name, seed) = {
            let all = seeds();
            let idx = rng.below(all.len());
            all[idx].clone()
        };
        let mut data = seed;
        let rounds = 1 + rng.below(6);
        for _ in 0..rounds {
            mutate(&mut rng, &mut data);
        }

        // The assertion is that this returns at all. A panic fails the test
        // with the iteration number, which reproduces exactly from the seed.
        // A malformed file is allowed to be an error, so only Ok is inspected.
        if let Ok(out) = sanitize(&data, &policy) {
            assert!(
                out.data.len() <= data.len().saturating_mul(2) + 4096,
                "iteration {iteration} ({name}): output grew implausibly, \
                 {} bytes in, {} out",
                data.len(),
                out.data.len()
            );
            if out.report.assurance == Assurance::None {
                assert_eq!(
                    out.data, data,
                    "iteration {iteration} ({name}): a file we could not parse \
                     must be returned byte for byte"
                );
            }
        }
    }
}

#[test]
fn sanitizing_twice_changes_nothing_the_second_time() {
    // A rebuild is meant to reach a fixed point: everything not on the keep-list
    // is gone after one pass, so a second pass has nothing left to take. If this
    // drifts, the rebuild is carrying something through that it does not
    // recognise as metadata.
    let policy = Policy::default();
    for (name, seed) in seeds() {
        let Ok(first) = sanitize(&seed, &policy) else { continue };
        if first.report.assurance == Assurance::None {
            continue;
        }
        let second = sanitize(&first.data, &policy).expect("a rebuilt file must still parse");
        assert_eq!(
            first.data, second.data,
            "{name}: sanitizing an already-sanitized file changed it again"
        );
        assert!(
            second.report.removed.is_empty(),
            "{name}: second pass still found {:?}",
            second.report.removed
        );
    }
}

#[test]
fn truncation_at_every_offset_is_survivable() {
    let policy = Policy::default();
    for (name, seed) in seeds() {
        for cut in 0..seed.len() {
            let _ = sanitize(&seed[..cut], &policy); // must not panic
        }
        let _ = name;
    }
}
