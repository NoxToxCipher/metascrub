//! Throw arbitrary bytes at the image washer.
//!
//! This is the crate that decodes attacker-controlled images, so it is the one
//! most worth fuzzing. The property is minimal and absolute: `wash` returns. It
//! may return an error (undecodable, too large), but it may not panic, abort,
//! hang, or exhaust memory. The size limit is left at its default so a
//! decompression bomb is refused rather than expanded.
#![no_main]

use libfuzzer_sys::fuzz_target;
use pixelwash::{wash, Settings};

fuzz_target!(|data: &[u8]| {
    let settings = Settings::default();
    // The result is intentionally ignored: reaching a return at all is the
    // property under test. A washed image that itself decodes is checked in the
    // crate's own unit tests.
    let _ = wash(data, &settings);
});
