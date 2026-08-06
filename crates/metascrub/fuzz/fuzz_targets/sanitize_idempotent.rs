//! A rebuild must reach a fixed point.
//!
//! Everything outside the keep-list is dropped on the first pass, so a second
//! pass has nothing left to remove. If a second pass finds more, the first one
//! carried something through that it failed to recognise as metadata, which is
//! exactly the silent failure an allowlist exists to prevent.
#![no_main]

use libfuzzer_sys::fuzz_target;
use metascrub::{sanitize, Assurance, Policy};

fuzz_target!(|data: &[u8]| {
    let policy = Policy::default();
    let Ok(first) = sanitize(data, &policy) else { return };
    if first.report.assurance == Assurance::None {
        return;
    }
    let Ok(second) = sanitize(&first.data, &policy) else {
        // A file we just rebuilt must still parse. Failing here means the
        // rebuild emits something the parser rejects.
        panic!("a sanitized file no longer parses");
    };
    assert_eq!(first.data, second.data, "sanitizing twice changed the file again");
});
