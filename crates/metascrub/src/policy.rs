//! What to keep. Everything not named here is dropped.

/// How to handle the EXIF orientation tag.
///
/// This is the one genuine conflict between privacy and not wrecking the file.
/// Phone cameras almost always record the sensor upright and describe the real
/// rotation in EXIF, so a photo whose EXIF is gone will often display sideways.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    /// Drop it with the rest of the EXIF. The image may display rotated.
    ///
    /// This is the default because it is the only option that leaves no EXIF
    /// block at all, and "no EXIF block" is a much easier property to verify
    /// than "an EXIF block containing only what we meant to keep".
    #[default]
    Drop,

    /// Rebuild a fresh EXIF block holding the orientation tag and nothing else.
    ///
    /// The output block is synthesized from scratch, not copied, so nothing can
    /// ride along inside it. The tag itself is one of eight values describing
    /// how to rotate the picture, which identifies a person about as well as
    /// the image being landscape does. The cost is that the file still has an
    /// EXIF marker in it, which a casual "does this have EXIF?" check will
    /// flag.
    PreserveMinimal,
}

/// How to handle embedded ICC colour profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorProfile {
    /// Drop the profile. Colours may shift in wide-gamut images.
    ///
    /// Default, on the general principle that a variable-length blob we do not
    /// parse is a place to hide things. Profiles carry a free-text description
    /// and a manufacturer/model pair, and a custom monitor profile can be
    /// distinctive enough to correlate files.
    #[default]
    Drop,

    /// Keep the profile verbatim so colour-managed viewers render correctly.
    Keep,
}

/// Sanitization settings.
///
/// [`Policy::default`] is the safe configuration; each field is a decision to
/// keep *more* than the safe minimum.
#[derive(Debug, Clone)]
pub struct Policy {
    /// EXIF orientation handling. See [`Orientation`].
    pub orientation: Orientation,

    /// ICC colour profile handling. See [`ColorProfile`].
    pub color_profile: ColorProfile,

    /// Sanitize images found inside documents (`word/media/*` and friends).
    ///
    /// On by default. A photo pasted into a report keeps its own EXIF, so a
    /// document sanitizer that stops at `docProps/` leaves the GPS coordinates
    /// sitting in the archive.
    pub recurse_embedded: bool,

    /// Refuse inputs longer than this many bytes.
    ///
    /// Parsers here are bounds-checked and allocate roughly the input size, but
    /// a ceiling is still worth having when the input arrives from the network.
    /// `None` disables the check.
    pub max_input_bytes: Option<u64>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            orientation: Orientation::default(),
            color_profile: ColorProfile::default(),
            recurse_embedded: true,
            max_input_bytes: None,
        }
    }
}

impl Policy {
    /// The strictest configuration: drop orientation, drop colour profiles.
    ///
    /// Identical to [`Policy::default`]; spelled out so calling code can say
    /// which posture it meant rather than relying on the default staying put.
    pub fn strict() -> Self {
        Self::default()
    }

    /// Keep the things that affect how the image looks: orientation and colour.
    ///
    /// Still removes GPS, timestamps, camera identity, maker notes, thumbnails
    /// and every unrecognised structure. Use when files are going somewhere
    /// that will display them and rotation matters more than the presence of a
    /// minimal EXIF block.
    pub fn preserve_appearance() -> Self {
        Self {
            orientation: Orientation::PreserveMinimal,
            color_profile: ColorProfile::Keep,
            ..Self::default()
        }
    }

    pub(crate) fn keep_icc(&self) -> bool {
        self.color_profile == ColorProfile::Keep
    }
}
