//! The explanatory text shown in the app's reference panel.
//!
//! Kept apart from the interface code because it is the part most likely to be
//! read closely, argued with, and corrected. Every claim here should be one the
//! project is willing to defend, and where something cannot be promised the
//! text says so rather than going quiet.

/// One thing the sanitizer removes, and why it is worth removing.
pub struct Item {
    pub name: &'static str,
    pub what: &'static str,
    pub why: &'static str,
}

pub const METADATA: &[Item] = &[
    Item {
        name: "EXIF",
        what: "A block of tags written by the camera: model, lens, serial \
               number, shutter count, exposure settings, the date and time to \
               the second, and often GPS coordinates.",
        why: "GPS is the obvious one, and it is accurate to a few metres, so a \
              single holiday photo can give away a home address. The rest is \
              quieter but adds up: the same camera body and lens across a set \
              of photographs ties them to one person even when nothing else \
              does.",
    },
    Item {
        name: "Maker note",
        what: "A vendor's private area inside EXIF, in an undocumented format \
               that differs between manufacturers and firmware versions.",
        why: "This is where the sensor serial number usually lives, along with \
              shutter actuation counts and internal settings. It is the single \
              most identifying field in a typical photograph, and because the \
              format is private, tools that only understand standard tags \
              often leave it in place.",
    },
    Item {
        name: "Thumbnail",
        what: "A small copy of the picture, stored inside the file so that \
               viewers can show a preview without decoding the whole image.",
        why: "Thumbnails are generated once and frequently not regenerated \
              after an edit. A photograph cropped to remove someone from the \
              frame can still carry the uncropped original inside it. The same \
              applies to blurred faces and painted-over details.",
    },
    Item {
        name: "XMP",
        what: "Adobe's metadata format, stored as XML. Written by editing \
               software and by some cameras.",
        why: "Carries the editing history, the software and version used, \
              ratings, keywords, and often the author's name or the licence \
              holder. It is also where catalogue identifiers live, which can \
              link a published image back to a specific library on a specific \
              machine.",
    },
    Item {
        name: "IPTC",
        what: "A press and publishing metadata block, often written by photo \
               management software.",
        why: "Designed to carry a byline, a copyright notice, contact details \
              and caption text. All useful for a wire service, all identifying \
              for anyone who did not intend to publish under their name.",
    },
    Item {
        name: "Colour profile",
        what: "An ICC profile describing how the file's colour numbers should \
               be interpreted.",
        why: "Mostly harmless, and kept if you ask for it. Removed by default \
              for two reasons: a profile carries a free-text description and a \
              device model, and a custom profile from a calibrated monitor can \
              be distinctive enough to link files. If your images are \
              wide-gamut and colour accuracy matters, turn it back on.",
    },
    Item {
        name: "Trailing data",
        what: "Anything appended after the point where the image format says \
               the file ends.",
        why: "Most tools stop reading at the end marker, which makes the space \
              after it a good hiding place. Some phone cameras genuinely store \
              a second, full-resolution photograph there. Whatever is in it, \
              it is not part of the picture and it travels with the file.",
    },
    Item {
        name: "Document info",
        what: "In PDFs and Office documents: title, subject, author, the \
               software that produced it, and creation and modification times.",
        why: "The author field is often a real name or a corporate username, \
              filled in automatically from the account the software was \
              installed under, without ever being shown to the person typing.",
    },
    Item {
        name: "Revision identifiers",
        what: "Random identifiers that word processors write into a document \
               and update as it is edited.",
        why: "Two documents sharing a revision identifier were edited in the \
              same session on the same machine. This links files that have \
              nothing else in common, and almost nobody knows the field \
              exists.",
    },
    Item {
        name: "Unrecognised structures",
        what: "Any block in the container that is not on the keep-list, \
               including vendor-private sections this tool has never seen.",
        why: "The reason the tool rebuilds files rather than editing them. A \
              tool that deletes the metadata it recognises will silently pass \
              through anything new, private, or deliberately hidden. Rebuilding \
              from a list of what to keep means the default for anything \
              unknown is to drop it.",
    },
];

/// Headed sections of the PRNU explainer.
pub struct Section {
    pub heading: &'static str,
    pub body: &'static str,
}

pub const PRNU: &[Section] = &[
    Section {
        heading: "What a sensor fingerprint is",
        body: "A camera sensor is a grid of millions of light-sensitive wells, \
               etched into silicon. Manufacturing cannot make them perfectly \
               identical, so each one responds to light a fraction differently \
               from its neighbours. Some read slightly bright, some slightly \
               dark, by a fraction of a percent.\n\n\
               That variation is fixed. It is decided when the sensor is \
               manufactured and it does not change over the life of the \
               camera. Every photograph the camera takes carries it, faintly \
               multiplied into the brightness of every pixel. It is called \
               Photo Response Non-Uniformity, or PRNU.\n\n\
               The practical consequence: it is a serial number written into \
               the picture itself rather than into the file's information \
               fields. Removing EXIF does not touch it. Nor does renaming the \
               file, screenshotting it, or sending it through an app that \
               strips metadata.",
    },
    Section {
        heading: "How it is used against someone",
        body: "An analyst estimates the noise-free version of a photograph, \
               subtracts it to leave a residual of fine detail, and correlates \
               that residual against a reference pattern for a camera. A strong \
               correlation says the photograph came from that sensor.\n\n\
               The important part is that they need the reference pattern \
               first. It is built either from the physical camera or from a \
               set of photographs already known to come from it. So this is a \
               linking attack rather than an identifying one. Nobody looks at \
               an anonymous photograph and derives a name from the pixels.\n\n\
               The realistic scenario is this: someone publishes work under \
               their own name, then publishes something anonymously, and both \
               were taken with the same camera. The analyst does not need to \
               identify the anonymous photograph. They only need to show it \
               came from the same sensor as the public ones.",
    },
    Section {
        heading: "Why denoising helps",
        body: "The pattern lives in the fine, high-frequency detail, which is \
               exactly what a denoiser targets.\n\n\
               The tools that detect PRNU work by denoising an image and \
               keeping the residual they subtracted. So denoising and keeping \
               the image instead is precisely the reverse operation, applied to \
               the part of the picture where the fingerprint is strongest.\n\n\
               It is done first, at full resolution, because that is where the \
               pattern is most estimable and therefore most worth attacking.",
    },
    Section {
        heading: "Why downscaling helps most",
        body: "Correlation depends on lining up each pixel of the photograph \
               with the corresponding point of the reference pattern. \
               Downscaling breaks that correspondence.\n\n\
               When an image is resized, each output pixel is mixed from \
               several input pixels. The fixed pattern is averaged together \
               with its neighbours and smeared across a new grid that no longer \
               matches the sensor's. Of the four operations here, this one \
               reduces correlation the most.\n\n\
               It is done after denoising so that resampling is the last thing \
               to touch the pixel grid.",
    },
    Section {
        heading: "Why a little noise is added",
        body: "After denoising and downscaling, some trace of the pattern \
               remains. Adding a small amount of fresh random noise lowers the \
               signal-to-noise ratio of any estimate an analyst can make from \
               the image.\n\n\
               It does not erase what is there. It makes what is there harder \
               to measure, which for a statistical test amounts to much the \
               same thing at the margin.",
    },
    Section {
        heading: "Why re-compressing is the weakest step",
        body: "The common advice is to compress and decompress a photograph to \
               destroy the fingerprint. That is the least effective of the \
               operations here.\n\n\
               PRNU survives moderate JPEG compression comfortably. Lossy \
               compression discards some high-frequency detail, which helps a \
               little, but it was never going to be sufficient on its own. It \
               is included as a final step rather than relied upon.",
    },
    Section {
        heading: "What does not work: colour",
        body: "Shifting the white balance, applying a colour cast, or changing \
               the gain of individual channels does nothing at all.\n\n\
               Detectors use normalised correlation, which divides out any \
               uniform scaling or offset before comparing. A global colour \
               change is exactly that kind of transformation, so it is removed \
               by the maths before the comparison happens. It costs colour \
               accuracy and buys no protection.\n\n\
               This is written down because it is an intuitive idea that \
               happens to be wrong, and it appears in a lot of advice online.",
    },
    Section {
        heading: "The honest limit",
        body: "This reduces correlation. It does not remove the fingerprint, \
               and nothing in this app will ever tell you that it has.\n\n\
               A forensic analyst with a strong reference pattern, many sample \
               images, and time can compensate for scaling factors and search \
               across crops. Against that, these operations raise the cost and \
               lower the confidence of a match. They do not make a match \
               impossible.\n\n\
               There is also a cost on your side. Every setting here degrades \
               the photograph: softer detail, fewer pixels, more compression. \
               That trade is yours to make, which is why this is switched off \
               until you switch it on.\n\n\
               If your safety depends on being unlinkable, the stronger \
               measure is not to publish from the same camera under two \
               identities in the first place.",
    },
];

/// A widely repeated claim, and what is actually the case.
pub struct Myth {
    pub claim: &'static str,
    pub reality: &'static str,
}

/// Things people are commonly told, which are wrong or half true.
///
/// Included because bad advice here is worse than no advice: someone who
/// believes a file is clean behaves as though it is.
pub const MYTHS: &[Myth] = &[
    Myth {
        claim: "Sending a photo through a messaging app removes everything.",
        reality: "Most large platforms do strip EXIF when you send a picture as \
                  a photo, so the location usually goes. Two catches. Sending \
                  the same file as a *document* or *file attachment*, which is \
                  an option on several apps, uploads the original untouched, \
                  metadata and all. And none of them touch the sensor pattern \
                  in the pixels, because that is not metadata.",
    },
    Myth {
        claim: "Taking a screenshot removes the metadata.",
        reality: "Largely true, and it also disrupts the sensor pattern, since \
                  you are capturing what the screen displayed rather than what \
                  the sensor recorded. But the screenshot carries its own new \
                  metadata, the quality is much worse than a proper clean copy, \
                  and the common mistake is to screenshot for safety and then \
                  send the original by accident.",
    },
    Myth {
        claim: "Renaming the file, or putting it in a zip, removes metadata.",
        reality: "Neither does anything at all. A filename is not part of the \
                  file's contents, and an archive preserves the file exactly so \
                  that it comes out the other side unchanged. That is the whole \
                  purpose of an archive.",
    },
    Myth {
        claim: "Converting to PNG strips the metadata.",
        reality: "PNG has its own metadata chunks, including a full EXIF chunk \
                  and free-text fields, and many converters copy the tags \
                  across rather than dropping them. Changing format is not the \
                  same as removing information.",
    },
    Myth {
        claim: "I turned location services off, so my photos are fine.",
        reality: "That removes GPS coordinates, which is the biggest single \
                  item, and it is worth doing. Everything else remains: camera \
                  model, lens, serial number in the maker note, exact timestamp, \
                  the embedded thumbnail, and editing history.",
    },
    Myth {
        claim: "Just compress the photo and the camera fingerprint is gone.",
        reality: "This is the most common piece of advice and the weakest of \
                  the useful operations. The pattern survives moderate JPEG \
                  compression comfortably. Compression helps at the margin; \
                  resizing and denoising do the real work.",
    },
    Myth {
        claim: "Changing the colours or white balance defeats the fingerprint.",
        reality: "It does nothing. The comparison is a normalised correlation, \
                  which divides out any uniform per-channel scaling or offset \
                  before the two are compared. A global colour change is \
                  removed by the arithmetic before it can have any effect.",
    },
    Myth {
        claim: "Cropping the photo defeats the fingerprint.",
        reality: "It helps, because it shifts the alignment the comparison \
                  depends on, but an analyst can search across possible crop \
                  positions. It also leaves the pattern intact in whatever \
                  pixels remain.",
    },
    Myth {
        claim: "Sensor fingerprinting is theoretical, or something from films.",
        reality: "It is a documented technique with a research literature going \
                  back to 2006 and use in real casework. Treating it as fiction \
                  is as much a mistake as treating it as infallible.",
    },
    Myth {
        claim: "Sensor fingerprinting means you can always be identified.",
        reality: "Equally wrong in the other direction. Matching requires a \
                  reference pattern for your specific camera, built from the \
                  physical device or from photographs already known to be \
                  yours. Without one, there is nothing to compare against. It \
                  links photographs to each other; it does not produce a name.",
    },
    Myth {
        claim: "This only matters if you are a journalist or an activist.",
        reality: "The most common real harm is domestic. A photograph posted \
                  publicly can carry the coordinates of the place it was taken, \
                  and a thumbnail can carry the version of the picture that was \
                  cropped for a reason.",
    },
];

/// What the published research actually supports, including where it limits
/// what this tool can claim.
pub const EVIDENCE: &[Section] = &[
    Section {
        heading: "Where the technique comes from",
        body: "Sensor fingerprinting was established by Lukáš, Fridrich and \
               Goljan in 'Digital Camera Identification from Sensor Pattern \
               Noise', published in IEEE Transactions on Information Forensics \
               and Security in 2006. It is the foundational paper and remains \
               the basis of the field.\n\n\
               Their method: build a reference pattern for a camera by taking \
               many photographs from it, denoising each one, keeping the \
               residual, and averaging those residuals so that the fixed \
               component reinforces while the random component cancels. A \
               questioned photograph is then denoised the same way and its \
               residual correlated against that reference.",
    },
    Section {
        heading: "Why a reference pattern is the whole story",
        body: "Because the reference is built by averaging across many images \
               from one camera, an analyst must already have either the device \
               or a body of photographs attributed to it.\n\n\
               This is the single most important fact about the threat, and it \
               is the one most often left out. The technique answers 'did these \
               come from the same sensor?' It does not answer 'whose camera is \
               this?' unless somebody has already supplied the answer.",
    },
    Section {
        heading: "What the evidence says about resizing",
        body: "This is where the honesty matters most, because resizing is the \
               main thing this tool does.\n\n\
               The literature is consistent: identification from downscaled \
               images remains possible, but performance degrades \
               significantly. Resizing acts as a low-pass filter, and different \
               scale factors preserve different parts of the signal. An analyst \
               who knows or guesses the scale factor can compensate for it.\n\n\
               So downscaling is the most effective operation available here, \
               and it is still not a defeat. 'Significantly degrades' is the \
               honest description, and it is the one this app uses.",
    },
    Section {
        heading: "What the evidence says about counter-forensics",
        body: "Counter-forensic methods against sensor fingerprints are an \
               active research area rather than a solved problem. Published \
               approaches include upscaling with one interpolation method and \
               downscaling with another, so that the pixel values are \
               plausible but no longer aligned with the original grid, and \
               various forms of noise suppression and injection.\n\n\
               None is presented in that literature as a guarantee. They are \
               described as making attribution harder, which is a different \
               claim, and it is the claim made here.",
    },
    Section {
        heading: "What is genuinely contested",
        body: "Reliability under real conditions is debated. Recent work asks \
               how well the technique holds up on modern smartphones, where \
               heavy computational processing, aggressive noise reduction and \
               digital stabilisation all interfere with the pattern before the \
               file is even written.\n\n\
               There is also ongoing discussion about whether the field has a \
               settled standard for casework. Anyone who tells you the answer \
               is simple, in either direction, is ahead of the evidence.",
    },
    Section {
        heading: "What this means for you",
        body: "Metadata removal is the part that is provable. The information \
               is in defined places, it is removed, and the result can be \
               checked with another tool.\n\n\
               Sensor fingerprint reduction is statistical. It lowers the \
               confidence of a match by an amount nobody can state precisely \
               for your specific photograph, camera and adversary.\n\n\
               Those are different kinds of claim and this app keeps them \
               visibly apart for that reason. If your safety depends on being \
               unlinkable, treat the fingerprint work as one layer among \
               several, not as the thing that solves it.",
    },
];

/// Shown the first time pixel washing is switched on.
pub const FIRST_USE: &str = "\
Your camera leaves a faint pattern in the pixels of every photograph it takes. \
It comes from tiny manufacturing differences between the light sensors, it is \
fixed for the life of the camera, and it is not metadata. Removing EXIF does \
nothing to it.\n\n\
It can be used to show that two photographs came from the same camera. That \
matters if you publish under your own name and also want to publish something \
anonymously.\n\n\
What this does: it denoises the image, makes it smaller, adds a little noise, \
and re-compresses it. Together these reduce how strongly the pattern can be \
matched.\n\n\
What it does not do: remove the pattern. Nobody can promise that. This lowers \
the confidence of a match; it does not make one impossible.\n\n\
It also costs image quality. Your photographs will be softer and smaller. That \
is why it stays off unless you turn it on.";
