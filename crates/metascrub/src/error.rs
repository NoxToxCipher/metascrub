//! Failure modes.
//!
//! Note what is deliberately *not* an error: a file in a format we do not
//! handle. That is an outcome ([`crate::Assurance::None`]), not a fault, and
//! conflating the two would push callers toward treating "unknown" as "clean".

/// Everything that can go wrong while sanitizing a file.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The container claimed a format but did not parse as one. A partial parse
    /// means a partial strip, so this fails rather than returning a file that
    /// was only half rebuilt.
    #[error("malformed {format}: {detail}")]
    Malformed {
        /// The container we were parsing as.
        format: &'static str,
        /// What specifically did not add up.
        detail: String,
    },

    /// The input is larger than [`crate::Policy::max_input_bytes`].
    #[error("input is {len} bytes, over the {limit}-byte limit")]
    TooLarge {
        /// Actual input length.
        len: u64,
        /// The configured ceiling.
        limit: u64,
    },

    /// The file is encrypted or password-protected, so its metadata cannot be
    /// reached without the key.
    #[error("{0} is encrypted; decrypt it before sanitizing")]
    Encrypted(&'static str),

    /// A structure we can read but cannot safely rewrite.
    #[error("unsupported {format}: {detail}")]
    Unsupported {
        /// The container we were parsing as.
        format: &'static str,
        /// Why we declined rather than guessed.
        detail: String,
    },

    /// Filesystem trouble from [`crate::sanitize_file`].
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub(crate) fn malformed(format: &'static str, detail: impl Into<String>) -> Self {
        Error::Malformed { format, detail: detail.into() }
    }

    // Only the archive backends decline a structure they can read; an
    // image-only build never reaches this.
    #[cfg_attr(not(feature = "ooxml"), allow(dead_code))]
    pub(crate) fn unsupported(format: &'static str, detail: impl Into<String>) -> Self {
        Error::Unsupported { format, detail: detail.into() }
    }
}
