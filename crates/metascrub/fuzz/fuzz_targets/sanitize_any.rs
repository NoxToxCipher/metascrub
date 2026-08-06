//! Throw arbitrary bytes at the format detector and every parser behind it.
//!
//! The property is minimal and absolute: `sanitize` returns. It may return an
//! error, it may return the input untouched with `Assurance::None`, but it may
//! not panic, abort, or fail to terminate. Release builds set
//! `panic = "abort"`, so a reachable panic is a process kill triggered by a
//! file someone else chose.
#![no_main]

use libfuzzer_sys::fuzz_target;
use metascrub::{sanitize, Assurance, Policy};

fuzz_target!(|data: &[u8]| {
    let policy = Policy::default();
    if let Ok(out) = sanitize(data, &policy) {
        // A format we could not take apart must come back byte for byte.
        // Anything else would be a silent claim to have cleaned it.
        if out.report.assurance == Assurance::None {
            assert_eq!(out.data, data);
        }
    }
});
