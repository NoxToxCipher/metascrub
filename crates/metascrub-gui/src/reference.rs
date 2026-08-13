//! The explanatory text shown in the app's reference panel.
//!
//! Kept apart from the interface code because it is the part most likely to be
//! read closely, argued with, and corrected. Every claim here should be one the
//! project is willing to defend, and where something cannot be promised the
//! text says so rather than going quiet.

use crate::i18n::Lang;

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
        why: "Meant to carry a byline, a copyright notice, contact details \
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
              after it a good hiding place. Some phone cameras store \
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

/// One file format, what it commonly carries, and how that identifies a person.
///
/// Written for the reader who does not already know that a holiday photo can
/// hold their home address or that a Word document names them. Each entry says
/// what the format tends to carry and, plainly, how that data is used to put a
/// name or a place to a file.
pub struct FileType {
    pub name: &'static str,
    /// What identifying data this format tends to carry.
    pub carries: &'static str,
    /// How that data is used to identify or link a person.
    pub identifies: &'static str,
}

pub const FILE_TYPES: &[FileType] = &[
    FileType {
        name: "JPEG photo (.jpg)",
        carries: "An EXIF block written by the camera or phone: GPS coordinates, \
                  the camera make, model and serial number, the lens, the exact \
                  date and time to the second, and a small thumbnail. Often an \
                  XMP block from editing software as well.",
        identifies: "The GPS is accurate to a few metres, so one holiday photo \
                     can give away a home. The serial number is the strong one: \
                     it is the same across every photo that camera has ever \
                     taken, so it ties an anonymous picture to the set you posted \
                     under your own name. The thumbnail is generated once and \
                     often not updated, so a photo cropped to remove someone can \
                     still carry the uncropped original inside it.",
    },
    FileType {
        name: "Phone photo (HEIC, HEIF, AVIF)",
        carries: "The same EXIF and XMP as a JPEG, and modern phones are \
                  thorough: precise GPS, the device model, capture settings, and \
                  sometimes a depth map or a burst of frames.",
        identifies: "Everything the JPEG case describes, and because it came \
                     straight off a phone with location on, the GPS is usually \
                     present and precise. The device model plus operating-system \
                     details narrow down whose phone it was.",
    },
    FileType {
        name: "PNG image (.png)",
        carries: "Text chunks that hold free-form comments, the software that \
                  wrote the file, a creation time, and sometimes a full EXIF \
                  block copied across from an original photo.",
        identifies: "People assume PNG is 'clean' because it is often used for \
                     screenshots, but converters frequently carry the original \
                     photo's EXIF, GPS and all, into the PNG. The software field \
                     fingerprints the tool and version you used.",
    },
    FileType {
        name: "WebP and GIF",
        carries: "WebP carries EXIF and XMP like a JPEG. GIF carries comment and \
                  application blocks, which have been used for author names, \
                  software strings and XMP.",
        identifies: "The same location and device story for WebP. For GIF it is \
                     usually the author or software string rather than GPS, but \
                     that still names a tool, an account, or a person.",
    },
    FileType {
        name: "TIFF (.tif)",
        carries: "TIFF is the container EXIF itself is built on, so it holds the \
                  full set: GPS, camera make, model and serial, timestamps, and \
                  an embedded thumbnail.",
        identifies: "The same as a JPEG, and TIFF is common for scans and \
                     professional work, where a scanner or camera serial and a \
                     precise timestamp can tie a document back to one machine.",
    },
    FileType {
        name: "Camera raw (CR2, CR3, NEF, ARW, RAF, DNG, ...)",
        carries: "Everything a JPEG carries, and more. A larger maker note holds \
                  the internal serial number and the shutter count. A full-size \
                  JPEG preview is embedded inside, with its own EXIF and GPS.",
        identifies: "A raw is a worse leak than the JPEG of the same shot, not a \
                     better one. The shutter count effectively numbers your \
                     photos in the order you took them. The maker note holds \
                     data a converter needs to develop the file, so this tool \
                     keeps it and the serial in it usually stays, which is why a \
                     raw can never be fully cleaned in place. To remove the \
                     serial, develop the raw to a JPEG and clean that.",
    },
    FileType {
        name: "SVG vector image (.svg)",
        carries: "SVG is XML, so it carries editor bookkeeping: the drawing \
                  program and its version, layer and window layout, sometimes a \
                  document path, a metadata block with author and licence, and \
                  references to external files.",
        identifies: "The editor fields and any embedded document path can carry \
                     a username or a folder structure that names you. An external \
                     reference makes the image fetch a resource from a server \
                     when someone opens it, which reports back to that server that it was \
                     viewed. Scripts in an SVG can run when it is opened in a \
                     browser.",
    },
    FileType {
        name: "XMP sidecar (.xmp)",
        carries: "A file that is nothing but metadata, written beside a photo by \
                  editing software: the author, copyright, GPS, the dates the \
                  photo was taken and edited, the full editing history, catalogue \
                  identifiers, and the camera serial number.",
        identifies: "People forget the sidecar exists and share it alongside the \
                     photo. It carries the identity the photo was cleaned of. The \
                     editing history links the file to a specific session on a \
                     specific machine, and the catalogue IDs link it to one photo \
                     library.",
    },
    FileType {
        name: "PDF document (.pdf)",
        carries: "A document-information block with the author, the software that \
                  produced it, and creation and modification times. Often an XMP \
                  block too, and a history of incremental edits.",
        identifies: "The author field is usually filled in automatically from the \
                     account the software was installed under, so it is often a \
                     real name or a corporate username the writer never typed or \
                     saw. The edit history can hold earlier versions of the file, \
                     which is how 'redacted' PDFs have leaked the text underneath \
                     the black boxes.",
    },
    FileType {
        name: "Word, Excel, PowerPoint, OpenDocument",
        carries: "Author and last-modified-by names, the company, total editing \
                  time, revision-save identifiers, tracked changes with the \
                  names of everyone who edited, template paths, and any images \
                  pasted in, which keep their own EXIF.",
        identifies: "The author and last-edited-by fields name real people or \
                     account usernames. Revision-save identifiers are random \
                     numbers that match between documents edited in the same \
                     session on the same machine, which links files that have \
                     nothing else in common. Tracked changes can expose who \
                     wrote what, and a pasted photo brings its own GPS along.",
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

/// What a camera raw is, and the specific ways this tool is limited on one.
///
/// Placed on its own because raws are the one format here that is cleaned by
/// editing in place rather than rebuilt, and because the word "raw" is very
/// widely misunderstood. Someone who believes their straight-from-the-camera
/// JPEG is a raw file, or that a raw is cleaned as thoroughly as a JPEG, is
/// making a decision on a false picture of what happened.
pub const RAW: &[Section] = &[
    Section {
        heading: "What a raw file actually is",
        body: "A raw file is the near-unprocessed readout of the camera's image \
               sensor, before the camera has turned it into a picture. It is not \
               a viewable image in the ordinary sense. It has no fixed colours, \
               no contrast curve and no sharpening applied; it is closer to a \
               photographic negative than to a finished print, and it has to be \
               'developed' by software before it looks like anything.\n\n\
               Because it holds the full sensor data with almost nothing thrown \
               away, a raw is large, and it can only be opened by software that \
               understands that particular camera's format. That is the whole \
               point of shooting raw: nothing has been decided or discarded yet, \
               so the photographer can make those choices later.",
    },
    Section {
        heading: "An unedited JPEG is not a raw",
        body: "This is the most common and most costly misunderstanding, so it \
               is worth being blunt. A JPEG straight off a phone or camera, that \
               you have never opened in an editor, is not a raw file. It has \
               already been developed inside the camera: the sensor data was \
               turned into colours, a contrast curve and sharpening were applied, \
               and the result was compressed and thrown away down to a fraction \
               of its original size. 'Unedited by you' is not the same as 'raw'.\n\n\
               The reliable way to tell them apart is the file type, not how the \
               picture looks. Raws have maker-specific extensions: Canon CR2 and \
               CR3, Nikon NEF and NRW, Sony ARW, Panasonic RW2, Olympus ORF, \
               Fujifilm RAF, Pentax PEF, the universal Adobe DNG, and a dozen \
               more. If the file ends in .jpg or .jpeg, it is a JPEG whatever \
               else is true of it. If in doubt, this tool tells you which format \
               it detected from the file's own contents, ignoring the name.",
    },
    Section {
        heading: "Why raws carry more to remove, not less",
        body: "A raw is a worse privacy risk than the JPEG of the same photo, \
               not a better one. It carries the full EXIF block, and its maker \
               note, where the sensor serial number and shutter count live, is \
               usually larger and more detailed than in a processed file.\n\n\
               A raw also almost always contains a complete JPEG preview of the \
               developed photograph, embedded inside it so that software can show \
               the shot without decoding the sensor data. That preview has its \
               own EXIF, its own GPS, and can even show a version of the scene \
               from before an edit. It is a file within the file, and it has to \
               be cleaned too. This tool finds and blanks the metadata inside \
               those embedded previews as well as in the raw's own tags.",
    },
    Section {
        heading: "Two levels of cleaning, and why raws are the lower one",
        body: "Ordinary images and documents are rebuilt from scratch: the tool \
               keeps a list of the parts worth keeping, copies only those into a \
               new file, and by construction nothing else survives. JPEG, PNG, \
               WebP, HEIC, AVIF, GIF, TIFF, PDF and Office files are cleaned this \
               way, to a 'Complete' result.\n\n\
               A raw cannot be treated that way. Its sensor image lives in \
               vendor-specific sub-sections whose layout is undocumented and \
               different for every manufacturer, and testing on real files from \
               many brands showed that rebuilding one hands back a file that no \
               longer opens. A raw is not something you can reshoot, so the tool \
               will not take that risk. Raws are cleaned in place instead: the \
               file is edited, not rebuilt, nothing is moved, and the length does \
               not change. This is always a 'Best effort' result.",
    },
    Section {
        heading: "Exactly what changes in a raw, and what does not",
        body: "Because the margin for a raw is narrow, the tool is precise about \
               all three categories, and it tells you the same thing on every \
               file it cleans.\n\n\
               REMOVED, overwritten with zeros: the GPS location; the date and \
               time the photo was taken and last changed; the owner and artist \
               names; any XMP or IPTC block; the standard serial-number and \
               image-ID fields; and the metadata inside the embedded preview \
               image. The report lists exactly which of these were found in your \
               file.\n\n\
               KEPT on purpose: the camera make and model, and the \
               manufacturer's maker note (explained below).\n\n\
               NOT TOUCHED: the sensor image data itself, and the file's ability \
               to be developed. The picture is bit-for-bit the same; only the \
               information around it changed.",
    },
    Section {
        heading: "Why the maker note is kept, and what that leaks",
        body: "The maker note is the manufacturer's private block. It does hold \
               the camera's internal serial number and its shutter count, which \
               are identifying. It would be good to remove.\n\n\
               But manufacturers also store the settings a raw converter needs to \
               develop the file in that same block: black and white levels, the \
               sensor's colour-filter layout, white balance, lens corrections. On \
               real files from Canon, Nikon, Olympus, Pentax and others, removing \
               the maker note either stopped the raw opening or changed how it \
               decoded. Corrupting the file is the one outcome this tool refuses, \
               so the maker note stays.\n\n\
               The honest consequence: on a raw, the internal serial number in \
               the maker note usually survives. How much identifying data remains \
               therefore depends on your camera, because some brands also write \
               the serial into a standard field, which is removed, while others \
               keep it only in the maker note, which is not. If that serial \
               matters to you, use the safe path below.",
    },
    Section {
        heading: "The safest path, and which cameras are covered",
        body: "If what you need is to share a picture rather than the raw \
               negative, develop the raw into a JPEG or PNG first and clean that. \
               A JPEG is rebuilt to a 'Complete' result and carries none of the \
               maker note, embedded preview, or vendor sub-sections a raw does. \
               That is the way to remove the serial the raw keeps.\n\n\
               The tool recognises raws from Canon (CR2, CR3), Nikon (NEF, NRW), \
               Sony (ARW, SR2), Fujifilm (RAF), Olympus and OM System (ORF), \
               Panasonic (RW2), Pentax (PEF), Leica, Samsung (SRW), Adobe (DNG), \
               Epson (ERF), GoPro (GPR), and the medium-format backs from \
               Hasselblad, Phase One and Leaf, among others. Each is identified \
               from its contents, never its file name. Sigma's X3F uses a layout \
               the tool does not yet parse, so it is left untouched and reported \
               as not cleaned, rather than cleaned badly.",
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
                  the same file as a document or a file attachment uploads the \
                  original untouched, metadata and all, and people choose that \
                  option for better quality without realising what else rides \
                  along. WhatsApp and Telegram both behave this way; Signal \
                  strips metadata in document mode too, which is a real \
                  difference between them. And none of them touch the sensor \
                  pattern in the pixels, because that is not metadata.",
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
        claim: "My photo is a raw file because I haven't edited it.",
        reality: "A photo you have not touched is still not a raw file. A JPEG \
                  straight from a phone or camera was already developed inside \
                  the camera: colours, contrast and sharpening applied, then \
                  compressed. A raw is the sensor data before any of that, in a \
                  maker-specific format (CR2, CR3, NEF, ARW, DNG, RW2, ORF, RAF \
                  and others) that needs special software to open. The file type \
                  decides it, not whether you have edited the picture. It \
                  matters here because a raw carries more to remove, and this \
                  tool can only clean a raw best-effort, not rebuild it. See \
                  'Camera raw files'.",
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
        claim: "The file is clean, so it cannot be traced back to me.",
        reality: "Cleaning the file handles what is inside the file. If you \
                  uploaded it while logged in, the platform has its own record \
                  of which account sent what and when, and that record is \
                  reachable by legal process regardless of how clean the file \
                  was. Some platforms also write their own identifier into \
                  images on the way through. See 'What this tool cannot reach'.",
    },
    Myth {
        claim: "Shrinking and re-compressing stops a platform recognising the image.",
        reality: "No. Platforms match images with perceptual hashes, which are \
                  built specifically to survive resizing, re-compression and \
                  small edits. The fingerprint reduction in this app disturbs \
                  the sensor pattern; it does not stop a platform seeing that \
                  two files are the same photograph. Different attack, \
                  different defence.",
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
               several forms of noise suppression and injection.\n\n\
               None is presented in that literature as a guarantee. They are \
               described as making attribution harder, which is a different \
               claim, and it is the claim made here.",
    },
    Section {
        heading: "Where researchers disagree",
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

/// Identifying information that exists outside the file, which no file-cleaning
/// tool can touch.
///
/// The most dangerous misunderstanding this app could create is that a clean
/// file is an anonymous one. Cleaning handles one channel. The account, the
/// connection and the platform's own records are separate channels, and they
/// are frequently the ones that matter.
pub const BEYOND_THE_FILE: &[Section] = &[
    Section {
        heading: "A clean file is not an anonymous upload",
        body: "Everything else in this panel is about information carried \
               inside a file. This section is about information held \
               somewhere else, describing the same file.\n\n\
               If you upload a photograph while logged in, the platform knows \
               which account uploaded it, when, and from which address, \
               regardless of how clean the file was. Removing the metadata \
               does not remove the upload record. It was never in the file to \
               begin with.\n\n\
               This tool cannot reach any of that, and no tool of this kind \
               can.",
    },
    Section {
        heading: "Platforms add their own identifiers",
        body: "Some services do not merely strip metadata, they write their \
               own in.\n\n\
               Facebook has embedded an identifier in the IPTC block of \
               uploaded images since around 2014, in the field intended for \
               transmission references, with values beginning 'FBMD'. It was \
               found by a security researcher in 2019, and the IPTC, who own \
               the standard being used, looked for documentation of the \
               practice and found none.\n\n\
               The practical effect: an image downloaded from the platform can \
               carry a marker that the platform can interpret, on a file that \
               otherwise looks stripped. This app removes IPTC blocks entirely, \
               so running a downloaded image through it removes that marker \
               too. What it cannot do is remove the copy the platform kept.",
    },
    Section {
        heading: "What legal process can obtain",
        body: "The exact mechanism is worth knowing, because it is often \
               described too loosely.\n\n\
               Under Meta's published guidelines, a subpoena in a criminal \
               investigation compels basic subscriber records: name, length of \
               service, email addresses, and recent login addresses. Compelling \
               the stored contents of an account, which includes messages, \
               photos and videos, requires a search warrant on a showing of \
               probable cause. Content is a higher bar than subscriber \
               details, not the same one.\n\n\
               Retention has a timing element as well. Meta preserves records \
               pending legal process, but a preservation request has to arrive \
               before the material is deleted. Data already gone is gone.\n\n\
               So the answer to 'can someone match this file back to whoever \
               sent it' is: with the right legal process, in the right \
               jurisdiction, within the retention window, frequently yes. That \
               has nothing to do with the file's metadata.",
    },
    Section {
        heading: "Perceptual hashing, and a limit of this tool",
        body: "Large platforms match images using perceptual hashes, which are \
               designed to survive exactly the changes that break an ordinary \
               checksum: resizing, re-compression, minor colour shifts, small \
               crops.\n\n\
               This has a direct consequence for the fingerprint reduction \
               offered here. Denoising, downscaling and re-encoding are \
               intended to disturb the sensor pattern, and perceptual hashing \
               is built to be indifferent to precisely those operations. A \
               washed copy will still match its original under that kind of \
               comparison.\n\n\
               These are different attacks with different defences. Nothing in \
               this app is a defence against a platform recognising that two \
               images are the same picture.",
    },
    Section {
        heading: "Every other copy",
        body: "A file you clean is one copy. The original is still in your \
               camera roll, most likely in a cloud backup, possibly in a \
               messaging app's own cache, and after you send it, on somebody \
               else's device where you have no say at all.\n\n\
               Cleaning a copy before sending is worth doing. It is not the \
               same as the information ceasing to exist.",
    },
    Section {
        heading: "What actually helps",
        body: "If the concern is the file, clean the file. That is what this \
               tool is for and it does it well.\n\n\
               If the concern is that an upload should not be traceable to \
               you, the file is the least of it. The account, the connection \
               it was made over, the payment method behind the account and the \
               device it came from all matter more, and none of them are \
               addressed here.\n\n\
               Being clear about that boundary is more useful than a tool that \
               implies it has covered everything.",
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

// ---------------------------------------------------------------------------
// Language-aware access. English is the source; each array is looked up by
// language. Sections not yet translated fall back to English, so the panel is
// never blank while a translation is in progress.
// ---------------------------------------------------------------------------

pub fn metadata(l: Lang) -> &'static [Item] {
    match l { Lang::Ru => METADATA_RU, Lang::My => METADATA_MY, Lang::La => METADATA_LA, _ => METADATA }
}
pub fn file_types(l: Lang) -> &'static [FileType] {
    match l { Lang::Ru => FILE_TYPES_RU, Lang::My => FILE_TYPES_MY, Lang::La => FILE_TYPES_LA, _ => FILE_TYPES }
}
pub fn raw(l: Lang) -> &'static [Section] {
    match l { Lang::Ru => RAW_RU, Lang::My => RAW_MY, Lang::La => RAW_LA, _ => RAW }
}
pub fn first_use(l: Lang) -> &'static str {
    match l { Lang::Ru => FIRST_USE_RU, Lang::My => FIRST_USE_MY, Lang::La => FIRST_USE_LA, _ => FIRST_USE }
}
pub fn prnu(l: Lang) -> &'static [Section] {
    match l { Lang::Ru => PRNU_RU, Lang::My => PRNU_MY, Lang::La => PRNU_LA, _ => PRNU }
}
pub fn myths(l: Lang) -> &'static [Myth] {
    match l { Lang::Ru => MYTHS_RU, Lang::My => MYTHS_MY, Lang::La => MYTHS_LA, _ => MYTHS }
}
pub fn evidence(l: Lang) -> &'static [Section] {
    match l { Lang::Ru => EVIDENCE_RU, Lang::My => EVIDENCE_MY, Lang::La => EVIDENCE_LA, _ => EVIDENCE }
}
pub fn beyond_the_file(l: Lang) -> &'static [Section] {
    match l { Lang::Ru => BEYOND_THE_FILE_RU, Lang::My => BEYOND_THE_FILE_MY, Lang::La => BEYOND_THE_FILE_LA, _ => BEYOND_THE_FILE }
}

// ===========================================================================
// Russian. Translated for meaning; a native review is still welcome. Technical
// tokens (EXIF, GPS, XMP, PDF, format names, PRNU) are kept in Latin on purpose.
// ===========================================================================

const METADATA_RU: &[Item] = &[
    Item {
        name: "EXIF",
        what: "Блок тегов, записанный камерой: модель, объектив, серийный номер, счётчик затвора, параметры экспозиции, дата и время с точностью до секунды, а часто и координаты GPS.",
        why: "GPS — самое очевидное, и он точен до нескольких метров, поэтому одна отпускная фотография может выдать домашний адрес. Остальное тише, но накапливается: одна и та же камера и объектив на наборе фотографий связывают их с одним человеком, даже когда больше ничто этого не делает.",
    },
    Item {
        name: "Maker note (заметка производителя)",
        what: "Приватная область производителя внутри EXIF, в недокументированном формате, который различается у разных производителей и версий прошивки.",
        why: "Именно здесь обычно находится серийный номер сенсора, вместе со счётчиком срабатываний затвора и внутренними настройками. Это самое идентифицирующее поле в типичной фотографии, и поскольку формат приватный, инструменты, понимающие только стандартные теги, часто оставляют его на месте.",
    },
    Item {
        name: "Миниатюра (thumbnail)",
        what: "Маленькая копия изображения, хранящаяся внутри файла, чтобы просмотрщики могли показать превью, не декодируя всё изображение.",
        why: "Миниатюры создаются один раз и часто не пересоздаются после редактирования. Фотография, обрезанная, чтобы убрать кого-то из кадра, всё ещё может нести внутри необрезанный оригинал. То же касается размытых лиц и закрашенных деталей.",
    },
    Item {
        name: "XMP",
        what: "Формат метаданных Adobe, хранящийся как XML. Записывается редакторами и некоторыми камерами.",
        why: "Несёт историю редактирования, использованные программу и её версию, оценки, ключевые слова, а часто имя автора или владельца лицензии. Здесь же находятся идентификаторы каталога, которые могут связать опубликованное изображение с конкретной библиотекой на конкретной машине.",
    },
    Item {
        name: "IPTC",
        what: "Блок метаданных для прессы и издательств, часто записываемый программами управления фотографиями.",
        why: "Предназначен для подписи автора, уведомления об авторских правах, контактных данных и текста подписи. Всё полезно для новостного агентства и всё идентифицирует любого, кто не собирался публиковаться под своим именем.",
    },
    Item {
        name: "Цветовой профиль",
        what: "Профиль ICC, описывающий, как следует интерпретировать числовые значения цвета в файле.",
        why: "В основном безвреден и сохраняется, если вы попросите. По умолчанию удаляется по двум причинам: профиль несёт текстовое описание и модель устройства, а нестандартный профиль с откалиброванного монитора может быть достаточно характерным, чтобы связать файлы. Если ваши изображения широкоохватные по цвету и точность цвета важна, включите его обратно.",
    },
    Item {
        name: "Хвостовые данные",
        what: "Всё, что добавлено после точки, где формат изображения объявляет конец файла.",
        why: "Большинство инструментов прекращают чтение на маркере конца, что делает пространство после него удобным тайником. Некоторые телефонные камеры хранят там вторую фотографию в полном разрешении. Что бы там ни было, это не часть картинки, и оно путешествует вместе с файлом.",
    },
    Item {
        name: "Сведения о документе",
        what: "В PDF и документах Office: заголовок, тема, автор, программа, создавшая его, а также время создания и изменения.",
        why: "Поле автора часто содержит настоящее имя или корпоративное имя пользователя, автоматически подставленное из учётной записи, под которой была установлена программа, при этом оно никогда не показывается тому, кто печатает.",
    },
    Item {
        name: "Идентификаторы правок",
        what: "Случайные идентификаторы, которые текстовые процессоры записывают в документ и обновляют по мере редактирования.",
        why: "Два документа с одинаковым идентификатором правки редактировались в одной сессии на одной машине. Это связывает файлы, у которых нет ничего общего, и почти никто не знает, что это поле существует.",
    },
    Item {
        name: "Нераспознанные структуры",
        what: "Любой блок в контейнере, которого нет в списке сохраняемого, включая приватные разделы производителей, которых этот инструмент никогда не видел.",
        why: "Причина, по которой инструмент пересобирает файлы, а не редактирует их. Инструмент, который удаляет только распознанные метаданные, молча пропустит всё новое, приватное или намеренно скрытое. Пересборка по списку того, что сохранить, означает, что по умолчанию всё неизвестное отбрасывается.",
    },
];

const FILE_TYPES_RU: &[FileType] = &[
    FileType {
        name: "Фотография JPEG (.jpg)",
        carries: "Блок EXIF, записанный камерой или телефоном: координаты GPS, марка, модель и серийный номер камеры, объектив, точные дата и время до секунды и маленькая миниатюра. Часто также блок XMP от программы редактирования.",
        identifies: "GPS точен до нескольких метров, поэтому одна отпускная фотография может выдать дом. Серийный номер — самое сильное: он одинаков на каждом снимке, который когда-либо делала эта камера, поэтому связывает анонимную картинку с набором, который вы выложили под своим именем. Миниатюра создаётся один раз и часто не обновляется, поэтому фотография, обрезанная, чтобы убрать кого-то, всё ещё может нести внутри необрезанный оригинал.",
    },
    FileType {
        name: "Фото с телефона (HEIC, HEIF, AVIF)",
        carries: "Те же EXIF и XMP, что и у JPEG, и современные телефоны тщательны: точный GPS, модель устройства, параметры съёмки, а иногда карта глубины или серия кадров.",
        identifies: "Всё, что описано для JPEG, и поскольку снимок пришёл прямо с телефона с включённой геолокацией, GPS обычно присутствует и точен. Модель устройства плюс детали операционной системы сужают, чей это был телефон.",
    },
    FileType {
        name: "Изображение PNG (.png)",
        carries: "Текстовые фрагменты со свободными комментариями, программой, записавшей файл, временем создания, а иногда полным блоком EXIF, скопированным из исходной фотографии.",
        identifies: "Люди считают PNG «чистым», потому что он часто используется для скриншотов, но конвертеры часто переносят EXIF исходной фотографии, вместе с GPS, в PNG. Поле программы выдаёт использованный вами инструмент и версию.",
    },
    FileType {
        name: "WebP и GIF",
        carries: "WebP несёт EXIF и XMP, как JPEG. GIF несёт блоки комментариев и приложений, которые использовались для имён авторов, строк программ и XMP.",
        identifies: "Та же история с местоположением и устройством для WebP. Для GIF это обычно строка автора или программы, а не GPS, но она всё равно называет инструмент, учётную запись или человека.",
    },
    FileType {
        name: "TIFF (.tif)",
        carries: "TIFF — это контейнер, на котором построен сам EXIF, поэтому он несёт полный набор: GPS, марку, модель и серийный номер камеры, метки времени и встроенную миниатюру.",
        identifies: "То же, что у JPEG, и TIFF распространён для сканов и профессиональной работы, где серийный номер сканера или камеры и точная метка времени могут привязать документ к одной машине.",
    },
    FileType {
        name: "Камерный raw (CR2, CR3, NEF, ARW, RAF, DNG, …)",
        carries: "Всё, что несёт JPEG, и больше. Более крупная maker note хранит внутренний серийный номер и счётчик затвора. Внутри встроено полноразмерное JPEG-превью со своими собственными EXIF и GPS.",
        identifies: "Raw — это худшая утечка, чем JPEG того же кадра, а не лучшая. Счётчик затвора фактически нумерует ваши снимки в порядке съёмки. Maker note хранит данные, нужные конвертеру для проявки файла, поэтому этот инструмент сохраняет её, и серийный номер в ней обычно остаётся — вот почему raw нельзя полностью очистить на месте. Чтобы удалить серийный номер, проявите raw в JPEG и очистите его.",
    },
    FileType {
        name: "Векторное изображение SVG (.svg)",
        carries: "SVG — это XML, поэтому он несёт служебные данные редактора: программу рисования и её версию, раскладку слоёв и окон, иногда путь к документу, блок метаданных с автором и лицензией, а также ссылки на внешние файлы.",
        identifies: "Поля редактора и любой встроенный путь к документу могут нести имя пользователя или структуру папок, называющую вас. Внешняя ссылка заставляет изображение запросить ресурс с сервера, когда кто-то его открывает, что сообщает тому серверу, что его просмотрели. Скрипты в SVG могут выполниться при открытии в браузере.",
    },
    FileType {
        name: "Файл-спутник XMP (.xmp)",
        carries: "Файл, состоящий из одних метаданных, записываемый рядом с фотографией программой редактирования: автор, авторские права, GPS, даты съёмки и редактирования, полная история редактирования, идентификаторы каталога и серийный номер камеры.",
        identifies: "Люди забывают, что спутник существует, и делятся им вместе с фотографией. Он несёт ту личность, от которой фотография была очищена. История редактирования связывает файл с конкретной сессией на конкретной машине, а идентификаторы каталога связывают его с одной фотобиблиотекой.",
    },
    FileType {
        name: "Документ PDF (.pdf)",
        carries: "Блок сведений о документе с автором, программой, создавшей его, и временем создания и изменения. Часто также блок XMP и история постепенных правок.",
        identifies: "Поле автора обычно заполняется автоматически из учётной записи, под которой была установлена программа, поэтому это часто настоящее имя или корпоративное имя пользователя, которое пишущий никогда не вводил и не видел. История правок может хранить более ранние версии файла — вот как «отредактированные» PDF выдавали текст под чёрными прямоугольниками.",
    },
    FileType {
        name: "Word, Excel, PowerPoint, OpenDocument",
        carries: "Имена автора и последнего редактора, компанию, общее время редактирования, идентификаторы сохранения правок, отслеживаемые изменения с именами всех, кто редактировал, пути к шаблонам и любые вставленные изображения, которые сохраняют свой EXIF.",
        identifies: "Поля автора и последнего редактора называют настоящих людей или имена учётных записей. Идентификаторы сохранения правок — это случайные числа, совпадающие между документами, отредактированными в одной сессии на одной машине, что связывает файлы, у которых нет ничего общего. Отслеживаемые изменения могут раскрыть, кто что написал, а вставленная фотография приносит с собой свой GPS.",
    },
];

const RAW_RU: &[Section] = &[
    Section {
        heading: "Что такое raw-файл на самом деле",
        body: "Raw-файл — это почти необработанное считывание с сенсора камеры, до того как камера превратила его в картинку. Это не просматриваемое изображение в обычном смысле. У него нет фиксированных цветов, нет кривой контраста и не применена резкость; он ближе к фотографическому негативу, чем к готовому отпечатку, и его нужно «проявить» программой, прежде чем он станет на что-то похож.\n\nПоскольку он хранит полные данные сенсора, почти ничего не отбрасывая, raw большой и может быть открыт только программой, понимающей формат именно этой камеры. В этом весь смысл съёмки в raw: ничего ещё не решено и не отброшено, поэтому фотограф может сделать этот выбор позже.",
    },
    Section {
        heading: "Нередактированный JPEG — это не raw",
        body: "Это самое распространённое и самое дорогое заблуждение, поэтому стоит сказать прямо. JPEG прямо с телефона или камеры, который вы никогда не открывали в редакторе, — это не raw-файл. Он уже проявлен внутри камеры: данные сенсора превращены в цвета, применены кривая контраста и резкость, а результат сжат и отброшен до доли исходного размера. «Не редактировано вами» — это не то же самое, что «raw».\n\nНадёжный способ их различить — тип файла, а не то, как выглядит картинка. У raw специфичные для производителя расширения: Canon CR2 и CR3, Nikon NEF и NRW, Sony ARW, Panasonic RW2, Olympus ORF, Fujifilm RAF, Pentax PEF, универсальный Adobe DNG и ещё десяток. Если файл заканчивается на .jpg или .jpeg, это JPEG, что бы ещё о нём ни было верно. Если сомневаетесь, инструмент сообщит, какой формат он определил по содержимому файла, игнорируя имя.",
    },
    Section {
        heading: "Почему raw несёт больше для удаления, а не меньше",
        body: "Raw — это больший риск для приватности, чем JPEG той же фотографии, а не меньший. Он несёт полный блок EXIF, а его maker note, где находятся серийный номер сенсора и счётчик затвора, обычно крупнее и подробнее, чем в обработанном файле.\n\nRaw также почти всегда содержит полное JPEG-превью проявленной фотографии, встроенное внутрь, чтобы программа могла показать снимок, не декодируя данные сенсора. У этого превью свой собственный EXIF, свой GPS, и оно может даже показывать версию сцены до редактирования. Это файл внутри файла, и его тоже нужно очистить. Этот инструмент находит и обнуляет метаданные внутри таких встроенных превью, а также в собственных тегах raw.",
    },
    Section {
        heading: "Два уровня очистки, и почему raw — на нижнем",
        body: "Обычные изображения и документы пересобираются с нуля: инструмент хранит список частей, которые стоит сохранить, копирует только их в новый файл, и по построению ничего другого не выживает. JPEG, PNG, WebP, HEIC, AVIF, GIF, TIFF, PDF и файлы Office очищаются так, до результата «Полностью».\n\nС raw так поступить нельзя. Его сенсорное изображение находится в специфичных для производителя подразделах, чья структура недокументирована и различна у каждого производителя, и тестирование на реальных файлах многих марок показало, что пересборка возвращает файл, который больше не открывается. Raw — это не то, что можно переснять, поэтому инструмент не пойдёт на такой риск. Вместо этого raw очищается на месте: файл редактируется, а не пересобирается, ничего не перемещается, и длина не меняется. Это всегда результат «По мере возможности».",
    },
    Section {
        heading: "Что именно меняется в raw, а что нет",
        body: "Поскольку запас для raw узкий, инструмент точен во всех трёх категориях и говорит вам одно и то же для каждого очищаемого файла.\n\nУДАЛЕНО, перезаписано нулями: местоположение GPS; дата и время съёмки и последнего изменения; имена владельца и автора; любой блок XMP или IPTC; стандартные поля серийного номера и идентификатора изображения; и метаданные внутри встроенного превью. Отчёт перечисляет, что именно из этого было найдено в вашем файле.\n\nСОХРАНЕНО намеренно: марка и модель камеры и maker note производителя (объяснено ниже).\n\nНЕ ТРОНУТО: сами данные сенсорного изображения и способность файла быть проявленным. Картинка бит в бит та же; изменилась только информация вокруг неё.",
    },
    Section {
        heading: "Почему maker note сохраняется, и что это выдаёт",
        body: "Maker note — это приватный блок производителя. Он действительно хранит внутренний серийный номер камеры и счётчик затвора, которые идентифицируют. Его было бы хорошо удалить.\n\nНо производители также хранят в том же блоке настройки, нужные raw-конвертеру для проявки файла: уровни чёрного и белого, раскладку цветового фильтра сенсора, баланс белого, коррекции объектива. На реальных файлах Canon, Nikon, Olympus, Pentax и других удаление maker note либо переставало открывать raw, либо меняло, как он декодируется. Порча файла — единственный исход, от которого инструмент отказывается, поэтому maker note остаётся.\n\nЧестное следствие: в raw внутренний серийный номер в maker note обычно выживает. Сколько идентифицирующих данных остаётся, зависит от вашей камеры, потому что некоторые марки также пишут серийный номер в стандартное поле, которое удаляется, тогда как другие хранят его только в maker note, которая — нет. Если этот серийный номер важен для вас, используйте безопасный путь ниже.",
    },
    Section {
        heading: "Самый безопасный путь, и какие камеры поддерживаются",
        body: "Если вам нужно поделиться картинкой, а не raw-негативом, сначала проявите raw в JPEG или PNG и очистите его. JPEG пересобирается до результата «Полностью» и не несёт ни maker note, ни встроенного превью, ни подразделов производителя, которые есть у raw. Это способ удалить серийный номер, который raw сохраняет.\n\nИнструмент распознаёт raw от Canon (CR2, CR3), Nikon (NEF, NRW), Sony (ARW, SR2), Fujifilm (RAF), Olympus и OM System (ORF), Panasonic (RW2), Pentax (PEF), Leica, Samsung (SRW), Adobe (DNG), Epson (ERF), GoPro (GPR), а также среднеформатные задники Hasselblad, Phase One и Leaf, среди прочих. Каждый определяется по содержимому, никогда по имени файла. X3F от Sigma использует структуру, которую инструмент пока не разбирает, поэтому он остаётся нетронутым и сообщается как неочищенный, а не очищенный плохо.",
    },
];

const FIRST_USE_RU: &str = "Ваша камера оставляет слабый узор в пикселях каждой сделанной фотографии. Он происходит из крошечных производственных различий между светочувствительными элементами, фиксирован на весь срок службы камеры и не является метаданными. Удаление EXIF на него никак не влияет.\n\nЕго можно использовать, чтобы показать, что две фотографии сделаны одной камерой. Это важно, если вы публикуете под своим именем и одновременно хотите опубликовать что-то анонимно.\n\nЧто это делает: устраняет шум на изображении, уменьшает его, добавляет немного шума и пересжимает. Вместе это снижает то, насколько сильно узор может быть сопоставлен.\n\nЧего это не делает: удаляет узор. Этого никто не может обещать. Это снижает уверенность в совпадении; оно не делает его невозможным.\n\nЭто также стоит качества изображения. Ваши фотографии станут мягче и меньше. Поэтому это выключено, пока вы не включите.";

const MYTHS_RU: &[Myth] = &[
    Myth {
        claim: "Отправка фото через мессенджер удаляет всё.",
        reality: "Большинство крупных платформ действительно вырезают EXIF, когда вы отправляете картинку как фото, поэтому местоположение обычно уходит. Две оговорки. Отправка того же файла как документа или вложения загружает оригинал нетронутым, со всеми метаданными, и люди выбирают этот вариант ради лучшего качества, не осознавая, что ещё едет вместе с ним. WhatsApp и Telegram оба ведут себя так; Signal вырезает метаданные и в режиме документа — это реальное различие между ними. И ни один из них не трогает сенсорный узор в пикселях, потому что это не метаданные.",
    },
    Myth {
        claim: "Скриншот удаляет метаданные.",
        reality: "В основном верно, и он также нарушает сенсорный узор, поскольку вы захватываете то, что показал экран, а не то, что записал сенсор. Но скриншот несёт свои новые метаданные, качество намного хуже, чем у правильной чистой копии, а типичная ошибка — сделать скриншот ради безопасности, а затем случайно отправить оригинал.",
    },
    Myth {
        claim: "Переименование файла или упаковка в zip удаляет метаданные.",
        reality: "Ни то, ни другое не делает ровно ничего. Имя файла не является частью его содержимого, а архив сохраняет файл в точности, чтобы он вышел с другого конца неизменным. В этом весь смысл архива.",
    },
    Myth {
        claim: "Моё фото — это raw-файл, потому что я его не редактировал.",
        reality: "Фотография, которую вы не трогали, всё равно не raw-файл. JPEG прямо с телефона или камеры уже был проявлен внутри камеры: применены цвета, контраст и резкость, затем сжат. Raw — это данные сенсора до всего этого, в специфичном для производителя формате (CR2, CR3, NEF, ARW, DNG, RW2, ORF, RAF и другие), для открытия которого нужна особая программа. Это решает тип файла, а не то, редактировали ли вы картинку. Здесь это важно, потому что raw несёт больше для удаления, и этот инструмент может очистить raw только по мере возможности, а не пересобрать. См. «Необработанные снимки (raw)».",
    },
    Myth {
        claim: "Конвертация в PNG удаляет метаданные.",
        reality: "У PNG свои собственные фрагменты метаданных, включая полный фрагмент EXIF и текстовые поля, и многие конвертеры копируют теги, а не отбрасывают их. Смена формата — это не то же самое, что удаление информации.",
    },
    Myth {
        claim: "Я отключил геолокацию, так что с моими фото всё в порядке.",
        reality: "Это убирает координаты GPS, самый крупный отдельный элемент, и это стоит делать. Всё остальное остаётся: модель камеры, объектив, серийный номер в maker note, точная метка времени, встроенная миниатюра и история редактирования.",
    },
    Myth {
        claim: "Просто сожми фото, и отпечаток камеры исчезнет.",
        reality: "Это самый распространённый совет и самая слабая из полезных операций. Узор спокойно переживает умеренное сжатие JPEG. Сжатие помогает на грани; изменение размера и шумоподавление делают настоящую работу.",
    },
    Myth {
        claim: "Изменение цветов или баланса белого побеждает отпечаток.",
        reality: "Оно не делает ничего. Сравнение — это нормализованная корреляция, которая делит любое равномерное поканальное масштабирование или смещение до того, как сравнивать. Глобальное изменение цвета убирается арифметикой прежде, чем оно сможет как-то повлиять.",
    },
    Myth {
        claim: "Обрезка фото побеждает отпечаток.",
        reality: "Она помогает, потому что сдвигает выравнивание, от которого зависит сравнение, но аналитик может перебрать возможные позиции обрезки. Она также оставляет узор нетронутым в тех пикселях, что остались.",
    },
    Myth {
        claim: "Идентификация по сенсору — это теория или что-то из фильмов.",
        reality: "Это документированная техника с исследовательской литературой, восходящей к 2006 году, и применением в реальных делах. Считать её выдумкой — такая же ошибка, как считать её непогрешимой.",
    },
    Myth {
        claim: "Идентификация по сенсору означает, что вас всегда можно опознать.",
        reality: "Столь же неверно в другую сторону. Для сопоставления нужен эталонный узор именно вашей камеры, построенный из физического устройства или из фотографий, уже известных как ваши. Без него не с чем сравнивать. Это связывает фотографии друг с другом; оно не выдаёт имя.",
    },
    Myth {
        claim: "Файл чист, значит, его нельзя отследить до меня.",
        reality: "Очистка файла разбирается с тем, что внутри файла. Если вы загрузили его, будучи в системе, у платформы есть своя запись о том, какая учётная запись что и когда отправила, и эта запись достижима по юридической процедуре независимо от того, насколько чист был файл. Некоторые платформы также вписывают свой идентификатор в изображения по пути. См. «Чего этот инструмент не может достичь».",
    },
    Myth {
        claim: "Уменьшение и пересжатие мешают платформе распознать изображение.",
        reality: "Нет. Платформы сопоставляют изображения перцептивными хешами, которые построены специально, чтобы переживать изменение размера, пересжатие и мелкие правки. Снижение отпечатка в этом приложении нарушает сенсорный узор; оно не мешает платформе видеть, что два файла — одна и та же фотография. Другая атака, другая защита.",
    },
    Myth {
        claim: "Это важно, только если вы журналист или активист.",
        reality: "Самый распространённый реальный вред — бытовой. Публично выложенная фотография может нести координаты места, где она сделана, а миниатюра может нести ту версию картинки, которая была обрезана не просто так.",
    },
];

const PRNU_RU: &[Section] = &[
    Section {
        heading: "Что такое отпечаток сенсора",
        body: "Сенсор камеры — это сетка из миллионов светочувствительных ячеек, вытравленных в кремнии. Производство не может сделать их идеально одинаковыми, поэтому каждая реагирует на свет чуть иначе, чем соседи. Одни считываются немного ярче, другие немного темнее, на долю процента.\n\nЭто отклонение фиксировано. Оно определяется при изготовлении сенсора и не меняется за весь срок службы камеры. Каждая фотография, которую делает камера, несёт его, слабо умноженным в яркость каждого пикселя. Это называется неоднородностью фотоотклика, или PRNU.\n\nПрактическое следствие: это серийный номер, вписанный в саму картинку, а не в информационные поля файла. Удаление EXIF его не трогает. Как и переименование файла, скриншот или отправка через приложение, вырезающее метаданные.",
    },
    Section {
        heading: "Как это используют против человека",
        body: "Аналитик оценивает версию фотографии без шума, вычитает её, оставляя остаток из мелких деталей, и коррелирует этот остаток с эталонным узором камеры. Сильная корреляция говорит, что фотография пришла с того сенсора.\n\nВажно то, что им сначала нужен эталонный узор. Он строится либо из физической камеры, либо из набора фотографий, уже известных как сделанные ею. Поэтому это атака связывания, а не идентификации. Никто не смотрит на анонимную фотографию и не выводит имя из пикселей.\n\nРеалистичный сценарий такой: кто-то публикует работы под своим именем, затем публикует что-то анонимно, и оба снимка сделаны одной камерой. Аналитику не нужно опознавать анонимную фотографию. Ему нужно лишь показать, что она пришла с того же сенсора, что и публичные.",
    },
    Section {
        heading: "Почему помогает шумоподавление",
        body: "Узор живёт в мелких, высокочастотных деталях — именно в том, на что нацелен шумоподавитель.\n\nИнструменты, обнаруживающие PRNU, работают, устраняя шум с изображения и сохраняя вычтенный остаток. Поэтому шумоподавление с сохранением самого изображения — это в точности обратная операция, применённая к той части картинки, где отпечаток сильнее всего.\n\nОно выполняется первым, в полном разрешении, потому что именно там узор наиболее оценим и потому наиболее уязвим для атаки.",
    },
    Section {
        heading: "Почему уменьшение помогает больше всего",
        body: "Корреляция зависит от совмещения каждого пикселя фотографии с соответствующей точкой эталонного узора. Уменьшение нарушает это соответствие.\n\nКогда изображение изменяется в размере, каждый выходной пиксель смешивается из нескольких входных. Фиксированный узор усредняется с соседями и размазывается по новой сетке, которая больше не совпадает с сенсорной. Из четырёх операций здесь эта снижает корреляцию сильнее всего.\n\nОна выполняется после шумоподавления, чтобы передискретизация была последним, что трогает сетку пикселей.",
    },
    Section {
        heading: "Почему добавляется немного шума",
        body: "После шумоподавления и уменьшения какой-то след узора остаётся. Добавление небольшого количества свежего случайного шума снижает отношение сигнал/шум для любой оценки, которую аналитик может сделать по изображению.\n\nОно не стирает то, что есть. Оно делает то, что есть, труднее измеримым, что для статистического теста на грани сводится примерно к тому же.",
    },
    Section {
        heading: "Почему пересжатие — самый слабый шаг",
        body: "Обычный совет — сжать и разжать фотографию, чтобы уничтожить отпечаток. Это наименее эффективная из операций здесь.\n\nPRNU спокойно переживает умеренное сжатие JPEG. Сжатие с потерями отбрасывает часть высокочастотных деталей, что немного помогает, но само по себе этого никогда не было бы достаточно. Оно включено как завершающий шаг, а не как то, на что полагаются.",
    },
    Section {
        heading: "Что не работает: цвет",
        body: "Сдвиг баланса белого, наложение цветового оттенка или изменение усиления отдельных каналов не делают ровно ничего.\n\nДетекторы используют нормализованную корреляцию, которая делит любое равномерное масштабирование или смещение до сравнения. Глобальное изменение цвета — именно такое преобразование, поэтому оно убирается математикой ещё до сравнения. Оно стоит точности цвета и не даёт никакой защиты.\n\nЭто записано, потому что это интуитивная идея, которая оказывается неверной, и она встречается во множестве советов в интернете.",
    },
    Section {
        heading: "Честный предел",
        body: "Это снижает корреляцию. Оно не удаляет отпечаток, и ничто в этом приложении никогда не скажет вам, что удалило.\n\nСудебный аналитик с сильным эталонным узором, множеством образцов и временем может компенсировать коэффициенты масштабирования и перебрать обрезки. Против этого данные операции повышают стоимость и снижают уверенность совпадения. Они не делают совпадение невозможным.\n\nЕсть также цена на вашей стороне. Каждая настройка здесь ухудшает фотографию: мягче детали, меньше пикселей, больше сжатия. Этот выбор за вами — поэтому это выключено, пока вы не включите.\n\nЕсли ваша безопасность зависит от невозможности связывания, более сильная мера — вообще не публиковать с одной камеры под двумя личностями.",
    },
];

const EVIDENCE_RU: &[Section] = &[
    Section {
        heading: "Откуда взялась эта техника",
        body: "Идентификацию по сенсору установили Лукаш, Фридрих и Голян в работе «Digital Camera Identification from Sensor Pattern Noise», опубликованной в IEEE Transactions on Information Forensics and Security в 2006 году. Это основополагающая статья, и она остаётся базой этой области.\n\nИх метод: построить эталонный узор камеры, сделав ею много фотографий, устранив шум с каждой, сохранив остаток и усреднив эти остатки так, чтобы фиксированная составляющая усиливалась, а случайная гасилась. Затем спорную фотографию очищают от шума тем же способом и коррелируют её остаток с этим эталоном.",
    },
    Section {
        heading: "Почему эталонный узор — это вся суть",
        body: "Поскольку эталон строится усреднением по многим изображениям с одной камеры, у аналитика уже должно быть либо устройство, либо массив фотографий, приписанных ему.\n\nЭто самый важный факт об угрозе, и его чаще всего опускают. Техника отвечает на вопрос «пришли ли они с одного сенсора?». Она не отвечает «чья это камера?», если только кто-то уже не дал ответ.",
    },
    Section {
        heading: "Что говорят данные об изменении размера",
        body: "Именно здесь честность важнее всего, потому что изменение размера — это главное, что делает этот инструмент.\n\nЛитература единодушна: идентификация с уменьшенных изображений остаётся возможной, но качество существенно падает. Изменение размера действует как фильтр нижних частот, и разные коэффициенты масштаба сохраняют разные части сигнала. Аналитик, который знает или угадывает коэффициент масштаба, может его компенсировать.\n\nТак что уменьшение — самая эффективная доступная здесь операция, и всё же это не поражение. «Существенно ухудшает» — честное описание, и именно его использует это приложение.",
    },
    Section {
        heading: "Что говорят данные о контркриминалистике",
        body: "Контркриминалистические методы против сенсорных отпечатков — это активная область исследований, а не решённая задача. Опубликованные подходы включают увеличение одним методом интерполяции и уменьшение другим, чтобы значения пикселей были правдоподобны, но больше не совпадали с исходной сеткой, а также несколько форм подавления и внесения шума.\n\nНи один не представлен в этой литературе как гарантия. Их описывают как затрудняющие атрибуцию — это другое утверждение, и именно оно делается здесь.",
    },
    Section {
        heading: "Где исследователи расходятся",
        body: "Надёжность в реальных условиях обсуждается. Недавние работы задаются вопросом, насколько хорошо техника держится на современных смартфонах, где интенсивная вычислительная обработка, агрессивное шумоподавление и цифровая стабилизация — всё это вмешивается в узор ещё до того, как файл записан.\n\nЕсть также продолжающаяся дискуссия о том, есть ли в этой области устоявшийся стандарт для судебной практики. Всякий, кто говорит вам, что ответ прост, в любую сторону, опережает данные.",
    },
    Section {
        heading: "Что это значит для вас",
        body: "Удаление метаданных — это доказуемая часть. Информация находится в определённых местах, она удаляется, и результат можно проверить другим инструментом.\n\nСнижение сенсорного отпечатка — статистическое. Оно снижает уверенность совпадения на величину, которую никто не может точно назвать для вашей конкретной фотографии, камеры и противника.\n\nЭто разные виды утверждений, и это приложение держит их видимо раздельно именно поэтому. Если ваша безопасность зависит от невозможности связывания, относитесь к работе с отпечатком как к одному слою из нескольких, а не как к тому, что решает задачу.",
    },
];

const BEYOND_THE_FILE_RU: &[Section] = &[
    Section {
        heading: "Чистый файл — это не анонимная загрузка",
        body: "Всё остальное на этой панели — об информации, которую несёт сам файл. Этот раздел — об информации, хранящейся где-то ещё и описывающей тот же файл.\n\nЕсли вы загружаете фотографию, будучи в системе, платформа знает, какая учётная запись её загрузила, когда и с какого адреса, независимо от того, насколько чист был файл. Удаление метаданных не удаляет запись о загрузке. Её изначально не было в файле.\n\nЭтот инструмент не может достать до всего этого, и ни один инструмент такого рода не может.",
    },
    Section {
        heading: "Платформы добавляют свои идентификаторы",
        body: "Некоторые сервисы не просто вырезают метаданные — они вписывают свои.\n\nFacebook встраивает идентификатор в блок IPTC загруженных изображений примерно с 2014 года, в поле, предназначенное для ссылок передачи, со значениями, начинающимися с «FBMD». Это обнаружил исследователь безопасности в 2019 году, а IPTC, которым принадлежит используемый стандарт, искали документацию этой практики и не нашли.\n\nПрактический эффект: изображение, скачанное с платформы, может нести маркер, который платформа может истолковать, на файле, который в остальном выглядит очищенным. Это приложение полностью удаляет блоки IPTC, поэтому прогон скачанного изображения через него удаляет и этот маркер. Чего оно не может — удалить копию, которую сохранила платформа.",
    },
    Section {
        heading: "Что можно получить по юридической процедуре",
        body: "Точный механизм стоит знать, потому что его часто описывают слишком вольно.\n\nСогласно опубликованным правилам Meta, повестка в рамках уголовного расследования обязывает выдать базовые данные подписчика: имя, срок обслуживания, адреса электронной почты и недавние адреса входа. Принуждение выдать хранимое содержимое учётной записи, включающее сообщения, фотографии и видео, требует ордера на обыск при наличии достаточных оснований. Содержимое — более высокая планка, чем данные подписчика, а не та же самая.\n\nУ хранения есть и временной элемент. Meta сохраняет записи в ожидании юридической процедуры, но запрос о сохранении должен прийти прежде, чем материал удалён. То, что уже исчезло, исчезло.\n\nТак что ответ на вопрос «может ли кто-то сопоставить этот файл с тем, кто его отправил» такой: при верной юридической процедуре, в верной юрисдикции, в пределах срока хранения — часто да. Это не имеет отношения к метаданным файла.",
    },
    Section {
        heading: "Перцептивное хеширование и предел этого инструмента",
        body: "Крупные платформы сопоставляют изображения перцептивными хешами, которые созданы, чтобы переживать именно те изменения, что ломают обычную контрольную сумму: изменение размера, пересжатие, мелкие сдвиги цвета, небольшие обрезки.\n\nЭто имеет прямое следствие для снижения отпечатка, предлагаемого здесь. Шумоподавление, уменьшение и перекодирование призваны нарушить сенсорный узор, а перцептивное хеширование построено так, чтобы быть безразличным именно к этим операциям. Обработанная копия всё равно совпадёт со своим оригиналом при таком сравнении.\n\nЭто разные атаки с разными защитами. Ничто в этом приложении не является защитой от того, что платформа распознает, что два изображения — одна и та же картинка.",
    },
    Section {
        heading: "Все остальные копии",
        body: "Файл, который вы очищаете, — это одна копия. Оригинал по-прежнему в вашей галерее, скорее всего в облачной резервной копии, возможно в собственном кэше мессенджера, а после отправки — на чужом устройстве, где вы вообще ничего не решаете.\n\nОчистить копию перед отправкой стоит. Это не то же самое, что исчезновение информации.",
    },
    Section {
        heading: "Что действительно помогает",
        body: "Если беспокоит файл — очистите файл. Для этого и предназначен этот инструмент, и он делает это хорошо.\n\nЕсли беспокоит то, что загрузку не должно быть можно отследить до вас, файл — наименьшая из проблем. Учётная запись, соединение, через которое она сделана, платёжный метод за учётной записью и устройство, с которого она пришла, — всё это значит больше, и ничто из этого здесь не решается.\n\nЯсность об этой границе полезнее, чем инструмент, намекающий, что он покрыл всё.",
    },
];

// ===========================================================================
// Burmese (draft). UNVERIFIED machine translation — a native translator checks
// it before release. Clean Unicode only; technical tokens kept in Latin.
// ===========================================================================

const METADATA_MY: &[Item] = &[
    Item {
        name: "EXIF",
        what: "ကင်မရာက ရေးသားသော tag အစုတစ်ခု— မော်ဒယ်၊ လင့်စ်၊ အမှတ်စဉ်နံပါတ်၊ ရှပ်တာအရေအတွက်၊ အလင်းဖမ်းဆက်တင်များ၊ စက္ကန့်အထိ တိကျသောရက်စွဲနှင့်အချိန်၊ နှင့် များသောအားဖြင့် GPS တည်နေရာ။",
        why: "GPS သည် အထင်ရှားဆုံးဖြစ်ပြီး မီတာအနည်းငယ်အထိ တိကျသဖြင့် အားလပ်ရက်ဓာတ်ပုံတစ်ပုံတည်းက အိမ်လိပ်စာကို ပေါက်ကြားစေနိုင်သည်။ ကျန်အရာများက ပိုတိတ်ဆိတ်သော်လည်း စုစည်းလာသည်— ဓာတ်ပုံအစုတစ်ခုတွင် တူညီသောကင်မရာကိုယ်ထည်နှင့် လင့်စ်သည် အခြားဘာမျှ မချိတ်ဆက်နိုင်သည့်အခါတွင်ပင် ၎င်းတို့ကို လူတစ်ဦးတည်းနှင့် ချိတ်ဆက်ပေးသည်။",
    },
    Item {
        name: "Maker note (ထုတ်လုပ်သူမှတ်စု)",
        what: "EXIF အတွင်းရှိ ရောင်းချသူ၏ သီးသန့်နေရာဖြစ်ပြီး ထုတ်လုပ်သူများနှင့် firmware ဗားရှင်းများအကြား ကွဲပြားသော စာရွက်စာတမ်းမပါသည့် ဖော်မတ်တစ်ခုဖြင့် ရှိသည်။",
        why: "ဤနေရာတွင် ဆင်ဆာအမှတ်စဉ်နံပါတ်သည် ရှပ်တာနှိပ်ချက်အရေအတွက်နှင့် အတွင်းဆက်တင်များနှင့်အတူ များသောအားဖြင့် ရှိနေသည်။ ၎င်းသည် ပုံမှန်ဓာတ်ပုံတစ်ပုံ၏ အဖော်ထုတ်နိုင်ဆုံးအကွက်ဖြစ်ပြီး ဖော်မတ်မှာ သီးသန့်ဖြစ်သဖြင့် စံ tag များကိုသာ နားလည်သည့်ကိရိယာများက ၎င်းကို မကြာခဏ ချန်ထားခဲ့တတ်သည်။",
    },
    Item {
        name: "ပုံသေး (thumbnail)",
        what: "ကြည့်ရှုသူများက ပုံတစ်ခုလုံးကို မ decode ဘဲ အစမ်းကြည့်ရှုနိုင်ရန် ဖိုင်အတွင်း သိမ်းထားသည့် ပုံ၏ မိတ္တူသေးတစ်ခု။",
        why: "ပုံသေးများကို တစ်ကြိမ်တည်း ဖန်တီးပြီး တည်းဖြတ်ပြီးနောက် မကြာခဏ ပြန်မဖန်တီးတော့ပါ။ တစ်စုံတစ်ဦးကို ဘောင်ထဲမှ ဖယ်ရှားရန် ဖြတ်တောက်ထားသော ဓာတ်ပုံသည် မဖြတ်တောက်ရသေးသော မူရင်းကို ၎င်းအတွင်း ဆက်လက်သယ်ဆောင်နေနိုင်သည်။ ဝါးထားသောမျက်နှာများနှင့် ဆေးသုတ်ဖုံးထားသော အသေးစိတ်များအတွက်လည်း အလားတူပင်။",
    },
    Item {
        name: "XMP",
        what: "Adobe ၏ မက်တာဒေတာဖော်မတ်ဖြစ်ပြီး XML အဖြစ် သိမ်းသည်။ တည်းဖြတ်ဆော့ဖ်ဝဲနှင့် အချို့ကင်မရာများက ရေးသားသည်။",
        why: "တည်းဖြတ်မှတ်တမ်း၊ အသုံးပြုသောဆော့ဖ်ဝဲနှင့်ဗားရှင်း၊ အဆင့်သတ်မှတ်ချက်များ၊ သော့ချက်စကားလုံးများ၊ နှင့် များသောအားဖြင့် ရေးသားသူ၏အမည် သို့မဟုတ် လိုင်စင်ပိုင်ရှင်ကို သယ်ဆောင်သည်။ ဤနေရာတွင် ကက်တလောက်အမှတ်အသားများလည်း ရှိပြီး ၎င်းတို့က ထုတ်ဝေထားသောပုံကို စက်တစ်ခုပေါ်ရှိ သီးခြားစာကြည့်တိုက်တစ်ခုနှင့် ပြန်ချိတ်ဆက်နိုင်သည်။",
    },
    Item {
        name: "IPTC",
        what: "သတင်းနှင့် ထုတ်ဝေရေး မက်တာဒေတာအပိုင်းတစ်ခုဖြစ်ပြီး များသောအားဖြင့် ဓာတ်ပုံစီမံခန့်ခွဲရေးဆော့ဖ်ဝဲက ရေးသားသည်။",
        why: "ရေးသားသူအမည်၊ မူပိုင်ခွင့်ကြေညာချက်၊ ဆက်သွယ်ရန်အချက်အလက်နှင့် စာတန်းစာသားတို့ကို သယ်ဆောင်ရန် ရည်ရွယ်သည်။ သတင်းဌာနတစ်ခုအတွက် အသုံးဝင်သည်၊ မိမိအမည်ဖြင့် ထုတ်ဝေရန် မရည်ရွယ်သူတိုင်းအတွက်မူ အားလုံးက ဖော်ထုတ်ပေးသည်။",
    },
    Item {
        name: "အရောင်ပရိုဖိုင်",
        what: "ဖိုင်၏ အရောင်ကိန်းဂဏန်းများကို မည်သို့အနက်ဖွင့်ရမည်ကို ဖော်ပြသည့် ICC ပရိုဖိုင်။",
        why: "အများအားဖြင့် အန္တရာယ်မရှိပြီး တောင်းဆိုပါက ထားရှိသည်။ အကြောင်းနှစ်ခုကြောင့် ပုံမှန်အားဖြင့် ဖယ်ရှားသည်— ပရိုဖိုင်သည် စာသားဖော်ပြချက်နှင့် စက်ပစ္စည်းမော်ဒယ်ကို သယ်ဆောင်ပြီး၊ ချိန်ညှိထားသောမော်နီတာမှ စိတ်ကြိုက်ပရိုဖိုင်တစ်ခုသည် ဖိုင်များကို ချိတ်ဆက်ရလောက်အောင် ထူးခြားနိုင်သည်။ သင့်ပုံများ အရောင်စုံပြီး အရောင်တိကျမှု အရေးကြီးပါက ၎င်းကို ပြန်ဖွင့်ပါ။",
    },
    Item {
        name: "နောက်ဆက်တွဲ ဒေတာ",
        what: "ပုံဖော်မတ်က ဖိုင်ပြီးဆုံးကြောင်း ဆိုသည့်နေရာနောက်တွင် ဖြည့်စွက်ထားသည့် မည်သည့်အရာမဆို။",
        why: "ကိရိယာအများစုသည် အဆုံးမှတ်တွင် ဖတ်ခြင်းရပ်သဖြင့် ၎င်းနောက်ကွက်လပ်သည် ကောင်းသောပုန်းအောင်းရာ ဖြစ်လာသည်။ အချို့ဖုန်းကင်မရာများက ထိုနေရာတွင် ပြည့်ဝသောဒုတိယဓာတ်ပုံတစ်ပုံကို သိမ်းသည်။ ၎င်းအတွင်း မည်သည့်အရာရှိစေ ၎င်းသည် ပုံ၏အစိတ်အပိုင်းမဟုတ်ဘဲ ဖိုင်နှင့်အတူ လိုက်ပါသွားသည်။",
    },
    Item {
        name: "စာရွက်စာတမ်း အချက်အလက်",
        what: "PDF နှင့် Office စာရွက်စာတမ်းများတွင်— ခေါင်းစဉ်၊ အကြောင်းအရာ၊ ရေးသားသူ၊ ၎င်းကို ထုတ်လုပ်သောဆော့ဖ်ဝဲ၊ နှင့် ဖန်တီးချိန်နှင့် ပြင်ဆင်ချိန်။",
        why: "ရေးသားသူအကွက်သည် ဆော့ဖ်ဝဲကို တပ်ဆင်ခဲ့သည့်အကောင့်မှ အလိုအလျောက်ဖြည့်ထားသည့် အမည်အစစ် သို့မဟုတ် ကုမ္ပဏီအသုံးပြုသူအမည် ဖြစ်တတ်ပြီး၊ စာရိုက်နေသူအား တစ်ခါမျှ မပြသဘဲ ဖြစ်နေတတ်သည်။",
    },
    Item {
        name: "ပြင်ဆင်မှု အမှတ်အသားများ",
        what: "စာစီစာရိုက်ပရိုဂရမ်များက စာရွက်စာတမ်းအတွင်း ရေးသွင်းပြီး တည်းဖြတ်သည်နှင့်အမျှ အပ်ဒိတ်လုပ်သည့် ကျပန်းအမှတ်အသားများ။",
        why: "ပြင်ဆင်မှုအမှတ်အသားတူညီသည့် စာရွက်စာတမ်းနှစ်ခုသည် တူညီသောစက်ပေါ်ရှိ တူညီသော session တစ်ခုတွင် တည်းဖြတ်ခဲ့ခြင်းဖြစ်သည်။ ၎င်းက အခြားဘာမျှ မတူညီသောဖိုင်များကို ချိတ်ဆက်ပေးပြီး ဤအကွက်ရှိမှန်း လူနီးပါး မသိကြပါ။",
    },
    Item {
        name: "မသိသော ဖွဲ့စည်းတည်ဆောက်ပုံများ",
        what: "သိမ်းရန်စာရင်းတွင် မပါသည့် ကွန်တိန်နာအတွင်းရှိ မည်သည့်အပိုင်းမဆို— ဤကိရိယာ တစ်ခါမျှ မမြင်ဖူးသည့် ရောင်းချသူသီးသန့်အပိုင်းများ အပါအဝင်။",
        why: "ကိရိယာက ဖိုင်များကို တည်းဖြတ်မည့်အစား ပြန်တည်ဆောက်ရသည့်အကြောင်းရင်း။ မိမိသိသောမက်တာဒေတာကို ဖျက်သည့်ကိရိယာသည် အသစ်၊ သီးသန့် သို့မဟုတ် တမင်ဖုံးကွယ်ထားသည့် မည်သည့်အရာကိုမဆို တိတ်တဆိတ် ဖြတ်သန်းခွင့်ပြုမည်။ သိမ်းရန်စာရင်းမှ ပြန်တည်ဆောက်ခြင်းက မသိသောအရာမှန်သမျှအတွက် ပုံမှန်အနေဖြင့် ပယ်ချရန် ဖြစ်စေသည်။",
    },
];

const FILE_TYPES_MY: &[FileType] = &[
    FileType {
        name: "JPEG ဓာတ်ပုံ (.jpg)",
        carries: "ကင်မရာ သို့မဟုတ် ဖုန်းက ရေးသားသော EXIF အပိုင်းတစ်ခု— GPS တည်နေရာ၊ ကင်မရာ အမှတ်တံဆိပ်၊ မော်ဒယ်နှင့် အမှတ်စဉ်နံပါတ်၊ လင့်စ်၊ စက္ကန့်အထိ တိကျသောရက်စွဲနှင့်အချိန်၊ နှင့် ပုံသေးတစ်ခု။ များသောအားဖြင့် တည်းဖြတ်ဆော့ဖ်ဝဲမှ XMP အပိုင်းတစ်ခုပါ။",
        identifies: "GPS သည် မီတာအနည်းငယ်အထိ တိကျသဖြင့် အားလပ်ရက်ဓာတ်ပုံတစ်ပုံက အိမ်ကို ပေါက်ကြားစေနိုင်သည်။ အမှတ်စဉ်နံပါတ်က အားအကောင်းဆုံး— ထိုကင်မရာ ရိုက်ဖူးသမျှ ဓာတ်ပုံတိုင်းတွင် တူညီသဖြင့် အမည်ဝှက်ပုံတစ်ခုကို သင်ကိုယ်ပိုင်အမည်ဖြင့် တင်ခဲ့သည့်အစုနှင့် ချိတ်ဆက်ပေးသည်။ ပုံသေးကို တစ်ကြိမ်တည်း ဖန်တီးပြီး မကြာခဏ မအပ်ဒိတ်သဖြင့် တစ်စုံတစ်ဦးကို ဖယ်ရှားရန် ဖြတ်ထားသောပုံသည် မဖြတ်ရသေးသောမူရင်းကို ၎င်းအတွင်း ဆက်လက်သယ်ဆောင်နိုင်သည်။",
    },
    FileType {
        name: "ဖုန်းဓာတ်ပုံ (HEIC, HEIF, AVIF)",
        carries: "JPEG ကဲ့သို့ တူညီသော EXIF နှင့် XMP၊ ခေတ်မီဖုန်းများက စေ့စပ်သည်— တိကျသော GPS၊ စက်ပစ္စည်းမော်ဒယ်၊ ရိုက်ကူးဆက်တင်များ၊ တစ်ခါတစ်ရံ အနက်မြေပုံ သို့မဟုတ် ဖရိမ်အစီအရီ။",
        identifies: "JPEG အခြေအနေတွင် ဖော်ပြထားသမျှနှင့် တည်နေရာဖွင့်ထားသောဖုန်းမှ တိုက်ရိုက်လာသဖြင့် GPS သည် များသောအားဖြင့် ရှိပြီး တိကျသည်။ စက်ပစ္စည်းမော်ဒယ်နှင့် operating-system အသေးစိတ်များက ၎င်းသည် မည်သူ့ဖုန်းဖြစ်ကြောင်း ကျဉ်းမြောင်းစေသည်။",
    },
    FileType {
        name: "PNG ပုံ (.png)",
        carries: "လွတ်လပ်သောမှတ်ချက်များ၊ ဖိုင်ရေးသောဆော့ဖ်ဝဲ၊ ဖန်တီးချိန်၊ နှင့် တစ်ခါတစ်ရံ မူရင်းဓာတ်ပုံမှ ကူးယူထားသော ပြည့်ဝသော EXIF အပိုင်းတစ်ခုကို ကိုင်ဆောင်သည့် စာသားအပိုင်းအစများ။",
        identifies: "PNG ကို screenshot များအတွက် မကြာခဏ သုံးသဖြင့် လူတို့က ၎င်းကို «သန့်» သည်ဟု ထင်ကြသည်၊ သို့သော် converter များက မူရင်းဓာတ်ပုံ၏ EXIF ကို GPS အပါအဝင် PNG ထဲသို့ မကြာခဏ ကူးယူတတ်သည်။ ဆော့ဖ်ဝဲအကွက်က သင်သုံးသောကိရိယာနှင့် ဗားရှင်းကို ဖော်ထုတ်သည်။",
    },
    FileType {
        name: "WebP နှင့် GIF",
        carries: "WebP သည် JPEG ကဲ့သို့ EXIF နှင့် XMP ကို သယ်ဆောင်သည်။ GIF သည် မှတ်ချက်နှင့် application အပိုင်းများကို သယ်ဆောင်ပြီး ၎င်းတို့ကို ရေးသားသူအမည်၊ ဆော့ဖ်ဝဲစာသားနှင့် XMP အတွက် သုံးခဲ့ကြသည်။",
        identifies: "WebP အတွက် တူညီသော တည်နေရာနှင့် စက်ပစ္စည်းဇာတ်လမ်း။ GIF အတွက်မူ များသောအားဖြင့် GPS မဟုတ်ဘဲ ရေးသားသူ သို့မဟုတ် ဆော့ဖ်ဝဲစာသားဖြစ်သည်၊ သို့သော် ၎င်းက ကိရိယာ၊ အကောင့် သို့မဟုတ် လူတစ်ဦးကို အမည်ဖော်နေဆဲဖြစ်သည်။",
    },
    FileType {
        name: "TIFF (.tif)",
        carries: "TIFF သည် EXIF ကိုယ်တိုင် တည်ဆောက်ထားသည့် ကွန်တိန်နာဖြစ်သဖြင့် အစုံအလင်ကို ကိုင်ဆောင်သည်— GPS၊ ကင်မရာ အမှတ်တံဆိပ်၊ မော်ဒယ်နှင့် အမှတ်စဉ်၊ အချိန်တံဆိပ်များနှင့် ထည့်သွင်းထားသောပုံသေး။",
        identifies: "JPEG နှင့် တူညီပြီး TIFF သည် စကင်များနှင့် ကျွမ်းကျင်လုပ်ငန်းများအတွက် အသုံးများသည်၊ ထိုနေရာတွင် စကင်နာ သို့မဟုတ် ကင်မရာအမှတ်စဉ်နှင့် တိကျသောအချိန်တံဆိပ်က စာရွက်စာတမ်းတစ်ခုကို စက်တစ်လုံးနှင့် ပြန်ချိတ်ဆက်နိုင်သည်။",
    },
    FileType {
        name: "ကင်မရာ raw (CR2, CR3, NEF, ARW, RAF, DNG, …)",
        carries: "JPEG သယ်ဆောင်သမျှနှင့် ပိုသည်။ ပိုကြီးသော maker note က အတွင်းအမှတ်စဉ်နံပါတ်နှင့် ရှပ်တာအရေအတွက်ကို ကိုင်ဆောင်သည်။ ၎င်း၏ကိုယ်ပိုင် EXIF နှင့် GPS ပါသော ပြည့်ဝသော JPEG အစမ်းကြည့်ပုံတစ်ခု အတွင်း၌ ထည့်သွင်းထားသည်။",
        identifies: "raw သည် တူညီသောရိုက်ချက်၏ JPEG ထက် ပိုဆိုးသော ပေါက်ကြားမှုဖြစ်ပြီး ပိုကောင်းသည်မဟုတ်။ ရှပ်တာအရေအတွက်က သင့်ဓာတ်ပုံများကို ရိုက်ခဲ့သည့်အစီအစဉ်အတိုင်း ထိထိရောက်ရောက် နံပါတ်တပ်ပေးသည်။ maker note က converter မှ ဖိုင်ကို develop ရန်လိုအပ်သောဒေတာကို ကိုင်ဆောင်သဖြင့် ဤကိရိယာက ၎င်းကို ထားရှိပြီး ၎င်းအတွင်းရှိ အမှတ်စဉ်က များသောအားဖြင့် ကျန်နေသည်— ထို့ကြောင့် raw ကို နေရာတွင်း၌ လုံးဝ သန့်စင်၍ မရ။ အမှတ်စဉ်ကို ဖယ်ရှားရန် raw ကို JPEG သို့ develop ပြီး ၎င်းကို သန့်စင်ပါ။",
    },
    FileType {
        name: "SVG vector ပုံ (.svg)",
        carries: "SVG သည် XML ဖြစ်သဖြင့် တည်းဖြတ်သူ၏ မှတ်တမ်းများကို သယ်ဆောင်သည်— ရေးဆွဲသည့်ပရိုဂရမ်နှင့်ဗားရှင်း၊ အလွှာနှင့် window အပြင်အဆင်၊ တစ်ခါတစ်ရံ စာရွက်စာတမ်းလမ်းကြောင်း၊ ရေးသားသူနှင့်လိုင်စင်ပါသော မက်တာဒေတာအပိုင်း၊ နှင့် ပြင်ပဖိုင်များသို့ ကိုးကားချက်များ။",
        identifies: "တည်းဖြတ်သူအကွက်များနှင့် ထည့်သွင်းထားသော စာရွက်စာတမ်းလမ်းကြောင်းက သင့်ကို အမည်ဖော်သည့် အသုံးပြုသူအမည် သို့မဟုတ် ဖိုလ်ဒါဖွဲ့စည်းပုံကို သယ်ဆောင်နိုင်သည်။ ပြင်ပကိုးကားချက်က တစ်စုံတစ်ဦးဖွင့်သည့်အခါ ပုံအား ဆာဗာတစ်ခုမှ အရင်းအမြစ်တစ်ခုကို ဆွဲယူစေပြီး ၎င်းကို ကြည့်ရှုကြောင်း ထိုဆာဗာသို့ ပြန်အစီရင်ခံသည်။ SVG ထဲရှိ script များသည် browser တွင်ဖွင့်သည့်အခါ လည်ပတ်နိုင်သည်။",
    },
    FileType {
        name: "XMP sidecar (.xmp)",
        carries: "မက်တာဒေတာသာ ဖြစ်သောဖိုင်တစ်ခုဖြစ်ပြီး တည်းဖြတ်ဆော့ဖ်ဝဲက ဓာတ်ပုံဘေးတွင် ရေးသားသည်— ရေးသားသူ၊ မူပိုင်ခွင့်၊ GPS၊ ဓာတ်ပုံရိုက်သည့်နှင့် တည်းဖြတ်သည့်ရက်စွဲများ၊ ပြည့်ဝသောတည်းဖြတ်မှတ်တမ်း၊ ကက်တလောက်အမှတ်အသားများ၊ နှင့် ကင်မရာအမှတ်စဉ်နံပါတ်။",
        identifies: "လူတို့သည် sidecar ရှိမှန်း မေ့ပြီး ဓာတ်ပုံနှင့်အတူ မျှဝေတတ်သည်။ ၎င်းက ဓာတ်ပုံမှ သန့်စင်ခဲ့သော ကိုယ်ပိုင်အထောက်အထားကို သယ်ဆောင်သည်။ တည်းဖြတ်မှတ်တမ်းက ဖိုင်ကို စက်တစ်လုံးပေါ်ရှိ သီးခြား session တစ်ခုနှင့် ချိတ်ဆက်ပြီး ကက်တလောက် ID များက ၎င်းကို ဓာတ်ပုံစာကြည့်တိုက်တစ်ခုနှင့် ချိတ်ဆက်သည်။",
    },
    FileType {
        name: "PDF စာရွက်စာတမ်း (.pdf)",
        carries: "ရေးသားသူ၊ ၎င်းကိုထုတ်လုပ်သောဆော့ဖ်ဝဲ၊ နှင့် ဖန်တီးချိန်နှင့် ပြင်ဆင်ချိန်ပါသော စာရွက်စာတမ်းအချက်အလက်အပိုင်းတစ်ခု။ များသောအားဖြင့် XMP အပိုင်းတစ်ခုနှင့် တစ်ဆင့်ချင်းတည်းဖြတ်မှုသမိုင်းပါ။",
        identifies: "ရေးသားသူအကွက်ကို ဆော့ဖ်ဝဲတပ်ဆင်ခဲ့သည့်အကောင့်မှ များသောအားဖြင့် အလိုအလျောက်ဖြည့်သဖြင့် ရေးသားသူ တစ်ခါမျှ မရိုက်ခဲ့၊ မမြင်ခဲ့သော အမည်အစစ် သို့မဟုတ် ကုမ္ပဏီအသုံးပြုသူအမည် ဖြစ်တတ်သည်။ တည်းဖြတ်မှတ်တမ်းက ဖိုင်၏ ယခင်ဗားရှင်းများကို ကိုင်ဆောင်နိုင်ပြီး ၎င်းက «ဖျောက်ထားသော» PDF များက အနက်ရောင်လေးထောင့်များအောက်ရှိ စာသားကို ပေါက်ကြားစေခဲ့သည့်နည်းလမ်းဖြစ်သည်။",
    },
    FileType {
        name: "Word, Excel, PowerPoint, OpenDocument",
        carries: "ရေးသားသူနှင့် နောက်ဆုံးပြင်ဆင်သူအမည်များ၊ ကုမ္ပဏီ၊ စုစုပေါင်းတည်းဖြတ်ချိန်၊ ပြင်ဆင်သိမ်းဆည်းအမှတ်အသားများ၊ တည်းဖြတ်သူတိုင်း၏အမည်ပါသော ခြေရာခံအပြောင်းအလဲများ၊ template လမ်းကြောင်းများ၊ နှင့် ကူးထည့်ထားသောပုံများ—၎င်းတို့သည် ကိုယ်ပိုင် EXIF ကို သိမ်းထားသည်။",
        identifies: "ရေးသားသူနှင့် နောက်ဆုံးတည်းဖြတ်သူအကွက်များက လူအစစ် သို့မဟုတ် အကောင့်အသုံးပြုသူအမည်များကို အမည်ဖော်သည်။ ပြင်ဆင်သိမ်းဆည်းအမှတ်အသားများသည် တူညီသောစက်ပေါ်ရှိ တူညီသော session တွင် တည်းဖြတ်ခဲ့သောစာရွက်စာတမ်းများအကြား ကိုက်ညီသည့် ကျပန်းနံပါတ်များဖြစ်ပြီး အခြားဘာမျှ မတူညီသောဖိုင်များကို ချိတ်ဆက်သည်။ ခြေရာခံအပြောင်းအလဲများက မည်သူဘာရေးသည်ကို ဖော်ထုတ်နိုင်ပြီး ကူးထည့်ထားသောဓာတ်ပုံက ကိုယ်ပိုင် GPS ကို ယူဆောင်လာသည်။",
    },
];

const FIRST_USE_MY: &str = "သင့်ကင်မရာသည် ရိုက်ကူးသမျှ ဓာတ်ပုံတိုင်း၏ ပစ်ဆယ်များတွင် သိမ်မွေ့သောပုံစံတစ်ခုကို ချန်ထားသည်။ ၎င်းသည် အလင်းဆင်ဆာများအကြား သေးငယ်သောထုတ်လုပ်မှုကွာခြားချက်များမှ လာပြီး ကင်မရာသက်တမ်းတစ်လျှောက် ပုံသေဖြစ်ကာ မက်တာဒေတာမဟုတ်ပါ။ EXIF ဖယ်ရှားခြင်းက ၎င်းကို ဘာမျှမလုပ်ပါ။\n\nဓာတ်ပုံနှစ်ပုံ တူညီသောကင်မရာမှ လာကြောင်း ပြသရန် ၎င်းကို သုံးနိုင်သည်။ သင်ကိုယ်ပိုင်အမည်ဖြင့် ထုတ်ဝေပြီး တစ်ခုခုကို အမည်ဝှက်ဖြင့်လည်း ထုတ်ဝေလိုပါက ၎င်းက အရေးကြီးသည်။\n\nဤသည်လုပ်ဆောင်ချက်— ပုံကို ဆူညံမှုဖယ်၊ သေးအောင်လုပ်၊ ဆူညံမှုအနည်းငယ်ထည့်၊ ပြန်ချုံ့သည်။ ၎င်းတို့အားလုံးက ပုံစံကို မည်မျှခိုင်မာစွာ ကိုက်ညီနိုင်သည်ကို လျှော့ချသည်။\n\nဤသည်မလုပ်ဆောင်သည့်အရာ— ပုံစံကို ဖယ်ရှားခြင်း။ ၎င်းကို မည်သူမျှ အာမမခံနိုင်ပါ။ ၎င်းက ကိုက်ညီမှု၏ယုံကြည်မှုကို လျှော့ချသည်၊ ၎င်းကို မဖြစ်နိုင်အောင် မလုပ်ပါ။\n\nပုံအရည်အသွေးလည်း ကုန်ကျသည်။ သင့်ဓာတ်ပုံများ ပိုနူးညံ့ပြီး ပိုသေးငယ်မည်။ ထို့ကြောင့် သင်ဖွင့်မှသာ ၎င်း ဖွင့်ထားသည်။";

const RAW_MY: &[Section] = &[
    Section {
        heading: "raw ဖိုင်ဆိုသည်မှာ အမှန်တကယ်ဘာလဲ",
        body: "raw ဖိုင်သည် ကင်မရာက ၎င်းကို ပုံအဖြစ် မပြောင်းမီ ကင်မရာ၏ ဆင်ဆာမှ နီးပါးမပြုပြင်ရသေးသော ဖတ်ချက်ဖြစ်သည်။ ၎င်းသည် သာမန်အဓိပ္ပာယ်ဖြင့် ကြည့်ရှုနိုင်သောပုံ မဟုတ်ပါ။ ၎င်းတွင် ပုံသေအရောင်များ မရှိ၊ ကွန်ထရပ်မျဉ်းကွေး မရှိ၊ ချွန်ထက်မှုလည်း မထည့်ရသေး— ၎င်းသည် ပြီးဆုံးပုံနှိပ်ချက်ထက် ဓာတ်ပုံ negative တစ်ခုနှင့် ပိုနီးစပ်ပြီး တစ်ခုခုနှင့်တူလာအောင် ဆော့ဖ်ဝဲဖြင့် develop လုပ်ရသည်။\n\nဆင်ဆာဒေတာအပြည့်ကို နီးပါးဘာမျှ မစွန့်ပစ်ဘဲ ကိုင်ဆောင်ထားသဖြင့် raw သည် ကြီးမားပြီး ထိုကင်မရာ၏ဖော်မတ်ကို နားလည်သောဆော့ဖ်ဝဲကသာ ဖွင့်နိုင်သည်။ ၎င်းသည် raw ရိုက်ခြင်း၏ အဓိကရည်ရွယ်ချက်— ဘာမျှ မဆုံးဖြတ်ရသေး၊ မစွန့်ရသေးသဖြင့် ဓာတ်ပုံဆရာက ထိုရွေးချယ်မှုများကို နောက်မှ ပြုလုပ်နိုင်သည်။",
    },
    Section {
        heading: "မတည်းဖြတ်ရသေးသော JPEG သည် raw မဟုတ်",
        body: "ဤသည်မှာ အဖြစ်များဆုံးနှင့် အကုန်အကျများဆုံး နားလည်မှုလွဲမှားခြင်းဖြစ်၍ ပွင့်ပွင့်လင်းလင်း ပြောသင့်သည်။ ဖုန်း သို့မဟုတ် ကင်မရာမှ တိုက်ရိုက်ရသော၊ တည်းဖြတ်ကိရိယာဖြင့် တစ်ခါမျှ မဖွင့်ဖူးသော JPEG သည် raw ဖိုင် မဟုတ်ပါ။ ၎င်းကို ကင်မရာအတွင်း develop ပြီးသား— ဆင်ဆာဒေတာကို အရောင်များအဖြစ် ပြောင်း၊ ကွန်ထရပ်မျဉ်းကွေးနှင့် ချွန်ထက်မှု ထည့်၊ ရလဒ်ကို ချုံ့ပြီး မူရင်းအရွယ်၏ အစိတ်အပိုင်းအထိ စွန့်ပစ်ပြီးဖြစ်သည်။ «သင်ကိုယ်တိုင် မတည်းဖြတ်ရသေး» သည် «raw» နှင့် မတူပါ။\n\n၎င်းတို့ကို ခွဲခြားရန် ယုံကြည်စိတ်ချရသောနည်းမှာ ပုံကဘယ်လိုမြင်ရသည်ထက် ဖိုင်အမျိုးအစားဖြစ်သည်။ raw တွင် ထုတ်လုပ်သူသီးသန့် extension များရှိသည်— Canon CR2 နှင့် CR3၊ Nikon NEF နှင့် NRW၊ Sony ARW၊ Panasonic RW2၊ Olympus ORF၊ Fujifilm RAF၊ Pentax PEF၊ အများသုံး Adobe DNG နှင့် အခြားဆယ်ခုကျော်။ ဖိုင်သည် .jpg သို့မဟုတ် .jpeg ဖြင့် ဆုံးပါက ၎င်းအကြောင်း အခြားဘာမှန်စေ ၎င်းသည် JPEG ဖြစ်သည်။ သံသယရှိပါက ကိရိယာက အမည်ကို လျစ်လျူရှုပြီး ဖိုင်၏ကိုယ်ပိုင်အကြောင်းအရာမှ မည်သည့်ဖော်မတ်ကို ရှာဖွေတွေ့ရှိသည်ကို သင့်အား ပြောပြသည်။",
    },
    Section {
        heading: "raw က ဖယ်ရှားရန် နည်းသည်မဟုတ်ဘဲ ပိုသယ်ဆောင်သည့်အကြောင်း",
        body: "raw သည် တူညီသောဓာတ်ပုံ၏ JPEG ထက် ပိုဆိုးသော ကိုယ်ရေးလုံခြုံရေးအန္တရာယ်ဖြစ်ပြီး ပိုကောင်းသည်မဟုတ်။ ၎င်းသည် EXIF အပိုင်းအပြည့်ကို သယ်ဆောင်ပြီး၊ ဆင်ဆာအမှတ်စဉ်နံပါတ်နှင့် ရှပ်တာအရေအတွက်ရှိသည့် ၎င်း၏ maker note သည် ပြုပြင်ပြီးဖိုင်ထက် များသောအားဖြင့် ပိုကြီး၍ ပိုအသေးစိတ်ဖြစ်သည်။\n\nraw သည် ဆင်ဆာဒေတာကို မ decode ဘဲ ဆော့ဖ်ဝဲက ရိုက်ချက်ကို ပြသနိုင်ရန် develop ပြီးဓာတ်ပုံ၏ ပြည့်ဝသော JPEG အစမ်းကြည့်ပုံတစ်ခုကို ၎င်းအတွင်း၌ နီးပါးအမြဲ ပါဝင်သည်။ ထိုအစမ်းကြည့်ပုံတွင် ကိုယ်ပိုင် EXIF၊ ကိုယ်ပိုင် GPS ရှိပြီး တည်းဖြတ်မီ မြင်ကွင်း၏ဗားရှင်းကိုပင် ပြသနိုင်သည်။ ၎င်းသည် ဖိုင်အတွင်းရှိ ဖိုင်တစ်ခုဖြစ်ပြီး ၎င်းကိုလည်း သန့်စင်ရမည်။ ဤကိရိယာက raw ၏ကိုယ်ပိုင် tag များအပြင် ထိုထည့်သွင်းထားသောအစမ်းကြည့်ပုံများအတွင်းရှိ မက်တာဒေတာကိုပါ ရှာဖွေ၍ သုညဖြင့် ဖျက်သည်။",
    },
    Section {
        heading: "သန့်စင်မှု နှစ်ဆင့်နှင့် raw က အောက်အဆင့်ဖြစ်ရသည့်အကြောင်း",
        body: "သာမန်ပုံများနှင့် စာရွက်စာတမ်းများကို အစမှ ပြန်တည်ဆောက်သည်— ကိရိယာက သိမ်းထိုက်သောအပိုင်းစာရင်းကို ထားရှိ၊ ၎င်းတို့ကိုသာ ဖိုင်အသစ်တစ်ခုသို့ ကူးယူပြီး တည်ဆောက်ပုံအရ အခြားဘာမျှ မကျန်စေ။ JPEG၊ PNG၊ WebP၊ HEIC၊ AVIF၊ GIF၊ TIFF၊ PDF နှင့် Office ဖိုင်များကို ဤနည်းဖြင့် «ပြီးပြည့်စုံ» ရလဒ်အထိ သန့်စင်သည်။\n\nraw ကို ထိုသို့ မကိုင်တွယ်နိုင်။ ၎င်း၏ဆင်ဆာပုံသည် ထုတ်လုပ်သူတိုင်းအတွက် ကွဲပြားပြီး စာရွက်စာတမ်းမပါသော အပြင်အဆင်ရှိသည့် ထုတ်လုပ်သူသီးသန့်ခွဲအပိုင်းများတွင် ရှိပြီး၊ အမှတ်တံဆိပ်များစွာ၏ တကယ့်ဖိုင်များပေါ်တွင် စမ်းသပ်ရာ ပြန်တည်ဆောက်ခြင်းက မဖွင့်နိုင်တော့သောဖိုင်ကို ပြန်ပေးသည်ကို ပြသခဲ့သည်။ raw သည် ပြန်ရိုက်နိုင်သောအရာမဟုတ်သဖြင့် ကိရိယာက ထိုအန္တရာယ်ကို မယူပါ။ ယင်းအစား raw ကို နေရာတွင်း၌ သန့်စင်သည်— ဖိုင်ကို တည်းဖြတ်၊ ပြန်မတည်ဆောက်၊ ဘာမျှ မရွှေ့၊ အရှည်လည်း မပြောင်း။ ဤသည် အမြဲ «တတ်နိုင်သမျှ» ရလဒ်ဖြစ်သည်။",
    },
    Section {
        heading: "raw တွင် အတိအကျ ဘာပြောင်းပြီး ဘာမပြောင်းသည်",
        body: "raw အတွက် ကြားချက် ကျဉ်းသဖြင့် ကိရိယာက အမျိုးအစားသုံးခုစလုံးအတွက် တိကျပြီး သန့်စင်သောဖိုင်တိုင်းတွင် တူညီသည့်အရာကို သင့်အား ပြောသည်။\n\nဖယ်ရှား၍ သုညဖြင့် ရေးဖျက်သည်— GPS တည်နေရာ၊ ဓာတ်ပုံရိုက်သည့်နှင့် နောက်ဆုံးပြောင်းသည့် ရက်စွဲနှင့်အချိန်၊ ပိုင်ရှင်နှင့် အနုပညာရှင်အမည်များ၊ မည်သည့် XMP သို့မဟုတ် IPTC အပိုင်းမဆို၊ စံ အမှတ်စဉ်နှင့် ပုံ-ID အကွက်များ၊ နှင့် ထည့်သွင်းထားသောအစမ်းကြည့်ပုံအတွင်းရှိ မက်တာဒေတာ။ အစီရင်ခံစာက ဤအထဲမှ သင့်ဖိုင်တွင် အတိအကျ ဘာတွေ့ရှိသည်ကို စာရင်းပြုစုသည်။\n\nတမင် ထားရှိသည်— ကင်မရာ အမှတ်တံဆိပ်နှင့် မော်ဒယ်၊ နှင့် ထုတ်လုပ်သူ၏ maker note (အောက်တွင် ရှင်းပြထားသည်)။\n\nမထိရ— ဆင်ဆာပုံဒေတာကိုယ်တိုင်နှင့် ဖိုင်၏ develop လုပ်နိုင်စွမ်း။ ပုံသည် bit တစ်ခုချင်းစီ အတူတူဖြစ်ပြီး ၎င်း၏ပတ်ဝန်းကျင်ရှိ အချက်အလက်သာ ပြောင်းသည်။",
    },
    Section {
        heading: "maker note ကို ထားရှိရသည့်အကြောင်းနှင့် ၎င်းပေါက်ကြားစေသည့်အရာ",
        body: "maker note သည် ထုတ်လုပ်သူ၏ သီးသန့်အပိုင်းဖြစ်သည်။ ၎င်းသည် ဖော်ထုတ်နိုင်သော ကင်မရာ၏အတွင်းအမှတ်စဉ်နံပါတ်နှင့် ရှပ်တာအရေအတွက်ကို အမှန်တကယ် ကိုင်ဆောင်သည်။ ၎င်းကို ဖယ်ရှားရလျှင် ကောင်းမည်။\n\nသို့သော် ထုတ်လုပ်သူများသည် raw converter မှ ဖိုင်ကို develop ရန် လိုအပ်သောဆက်တင်များကိုပါ ထိုအပိုင်းတွင်ပင် သိမ်းသည်— အနက်နှင့်အဖြူအဆင့်များ၊ ဆင်ဆာ၏ အရောင်စစ်ထုတ်အပြင်အဆင်၊ အဖြူ balance၊ လင့်စ်ပြင်ဆင်ချက်များ။ Canon၊ Nikon၊ Olympus၊ Pentax နှင့် အခြားများ၏ တကယ့်ဖိုင်များပေါ်တွင် maker note ကို ဖယ်ရှားခြင်းက raw ကို မဖွင့်နိုင်တော့စေ သို့မဟုတ် ၎င်း decode ပုံကို ပြောင်းစေခဲ့သည်။ ဖိုင်ကို ပျက်စီးစေခြင်းသည် ဤကိရိယာ ငြင်းဆိုသောတစ်ခုတည်းသောရလဒ်ဖြစ်သဖြင့် maker note ကျန်နေသည်။\n\nရိုးသားသောအကျိုးဆက်— raw တွင် maker note ရှိ အတွင်းအမှတ်စဉ်နံပါတ်က များသောအားဖြင့် ကျန်နေသည်။ ဖော်ထုတ်နိုင်သောဒေတာ မည်မျှကျန်သည်ကို သင့်ကင်မရာပေါ်တွင် မူတည်သည်၊ အဘယ်ကြောင့်ဆိုသော် အချို့အမှတ်တံဆိပ်များက အမှတ်စဉ်ကို ဖယ်ရှားခံရသည့် စံအကွက်တွင်ပါ ရေးသော်လည်း အခြားများက ၎င်းကို ဖယ်ရှားခြင်းမခံရသည့် maker note တွင်သာ သိမ်းသောကြောင့်။ ထိုအမှတ်စဉ်က သင့်အတွက် အရေးကြီးပါက အောက်ပါ လုံခြုံသောနည်းလမ်းကို သုံးပါ။",
    },
    Section {
        heading: "အလုံခြုံဆုံးနည်းလမ်းနှင့် မည်သည့်ကင်မရာများ ပါဝင်သည်",
        body: "raw negative ထက် ပုံတစ်ပုံကို မျှဝေရန်သာ လိုအပ်ပါက raw ကို JPEG သို့မဟုတ် PNG သို့ ဦးစွာ develop ပြီး ၎င်းကို သန့်စင်ပါ။ JPEG ကို «ပြီးပြည့်စုံ» ရလဒ်အထိ ပြန်တည်ဆောက်ပြီး raw တွင်ရှိသော maker note၊ ထည့်သွင်းအစမ်းကြည့်ပုံ သို့မဟုတ် ထုတ်လုပ်သူခွဲအပိုင်းများ တစ်ခုမျှ မသယ်ဆောင်ပါ။ ၎င်းသည် raw ထားရှိသောအမှတ်စဉ်ကို ဖယ်ရှားရန်နည်းလမ်းဖြစ်သည်။\n\nကိရိယာက Canon (CR2, CR3)၊ Nikon (NEF, NRW)၊ Sony (ARW, SR2)၊ Fujifilm (RAF)၊ Olympus နှင့် OM System (ORF)၊ Panasonic (RW2)၊ Pentax (PEF)၊ Leica၊ Samsung (SRW)၊ Adobe (DNG)၊ Epson (ERF)၊ GoPro (GPR) နှင့် Hasselblad၊ Phase One နှင့် Leaf တို့၏ အလတ်စားဖော်မတ် back များ အပါအဝင် raw များကို မှတ်မိသည်။ တစ်ခုစီကို ၎င်း၏ ဖိုင်အမည်ဖြင့် မဟုတ်ဘဲ အကြောင်းအရာဖြင့် ဖော်ထုတ်သည်။ Sigma ၏ X3F သည် ကိရိယာ မခွဲခြမ်းရသေးသော အပြင်အဆင်ကို သုံးသဖြင့် ၎င်းကို မထိဘဲ ချန်ထားပြီး မကောင်းစွာသန့်စင်ခြင်းထက် မသန့်စင်ရသေးဟု အစီရင်ခံသည်။",
    },
];

const PRNU_MY: &[Section] = &[
    Section {
        heading: "ဆင်ဆာလက်ဗွေဆိုသည်မှာ ဘာလဲ",
        body: "ကင်မရာဆင်ဆာသည် ဆီလီကွန်ထဲ ထွင်းထားသော အလင်းခံ တွင်းငယ်သန်းပေါင်းများစွာ၏ ကွက်တစ်ခုဖြစ်သည်။ ထုတ်လုပ်မှုက ၎င်းတို့ကို ပြီးပြည့်စုံစွာ တူညီအောင် မလုပ်နိုင်သဖြင့် တစ်ခုစီသည် အနီးအနားရှိသူများနှင့် အနည်းငယ်ကွဲပြားစွာ အလင်းကို တုံ့ပြန်သည်။ အချို့က အနည်းငယ်တောက်၊ အချို့က အနည်းငယ်မှောင်— ရာခိုင်နှုန်းအစိတ်အပိုင်းအားဖြင့်။\n\nထိုကွဲပြားမှုသည် ပုံသေဖြစ်သည်။ ၎င်းကို ဆင်ဆာ ထုတ်လုပ်သည့်အခါ ဆုံးဖြတ်ပြီး ကင်မရာသက်တမ်းတစ်လျှောက် မပြောင်းပါ။ ကင်မရာရိုက်သမျှ ဓာတ်ပုံတိုင်းသည် ၎င်းကို ပစ်ဆယ်တိုင်း၏ တောက်ပမှုထဲသို့ သိမ်မွေ့စွာ မြှောက်၍ သယ်ဆောင်သည်။ ၎င်းကို Photo Response Non-Uniformity သို့မဟုတ် PRNU ဟု ခေါ်သည်။\n\nလက်တွေ့အကျိုးဆက်— ၎င်းသည် ဖိုင်၏အချက်အလက်အကွက်များထဲသို့ မဟုတ်ဘဲ ပုံကိုယ်တိုင်ထဲသို့ ရေးထားသော အမှတ်စဉ်နံပါတ်ဖြစ်သည်။ EXIF ဖယ်ရှားခြင်းက ၎င်းကို မထိ။ ဖိုင်အမည်ပြောင်းခြင်း၊ screenshot ရိုက်ခြင်း သို့မဟုတ် မက်တာဒေတာဖယ်ရှားသော app မှတစ်ဆင့် ပို့ခြင်းကလည်း မထိ။",
    },
    Section {
        heading: "တစ်စုံတစ်ဦးကို ဆန့်ကျင်ရန် မည်သို့သုံးသည်",
        body: "ခွဲခြမ်းသူက ဓာတ်ပုံ၏ ဆူညံမှုကင်းသောဗားရှင်းကို ခန့်မှန်း၊ ၎င်းကို နုတ်၍ အသေးစိတ်ကျန်ရှိချက်တစ်ခု ချန်ထား၊ ထိုကျန်ရှိချက်ကို ကင်မရာတစ်လုံး၏ ကိုးကားပုံစံနှင့် ဆက်စပ်သည်။ ခိုင်မာသောဆက်စပ်မှုက ဓာတ်ပုံသည် ထိုဆင်ဆာမှ လာသည်ဟု ဆိုသည်။\n\nအရေးကြီးသည်မှာ သူတို့ ကိုးကားပုံစံကို ဦးစွာ လိုအပ်ခြင်းဖြစ်သည်။ ၎င်းကို ရုပ်ပိုင်းဆိုင်ရာကင်မရာမှ သို့မဟုတ် ၎င်းမှလာသည်ဟု သိရှိပြီးသော ဓာတ်ပုံအစုမှ တည်ဆောက်သည်။ ထို့ကြောင့် ၎င်းသည် ဖော်ထုတ်မှုတိုက်ခိုက်မှုထက် ချိတ်ဆက်မှုတိုက်ခိုက်မှုဖြစ်သည်။ မည်သူမျှ အမည်ဝှက်ဓာတ်ပုံကို ကြည့်၍ ပစ်ဆယ်များမှ အမည်တစ်ခု မထုတ်ယူနိုင်။\n\nလက်တွေ့ကျသောအခြေအနေမှာ— တစ်စုံတစ်ဦးက ကိုယ်ပိုင်အမည်ဖြင့် အလုပ်တစ်ခုကို ထုတ်ဝေ၊ ပြီးမှ တစ်ခုခုကို အမည်ဝှက်ဖြင့် ထုတ်ဝေ၊ နှစ်ခုစလုံးကို တူညီသောကင်မရာဖြင့် ရိုက်ခဲ့ခြင်းဖြစ်သည်။ ခွဲခြမ်းသူက အမည်ဝှက်ဓာတ်ပုံကို ဖော်ထုတ်ရန် မလို။ ၎င်းသည် အများသိဓာတ်ပုံများနှင့် တူညီသောဆင်ဆာမှ လာကြောင်း ပြရန်သာ လိုသည်။",
    },
    Section {
        heading: "ဆူညံမှုဖယ်ခြင်းက အဘယ်ကြောင့် အထောက်အကူပြုသည်",
        body: "ပုံစံသည် အသေးစိတ်၊ ကြိမ်နှုန်းမြင့်အသေးစိတ်ထဲတွင် နေထိုင်သည်— ၎င်းသည် ဆူညံမှုဖယ်ကိရိယာ ချိန်ရွယ်သည့်အရာအတိအကျဖြစ်သည်။\n\nPRNU ကို ရှာဖွေသောကိရိယာများသည် ပုံကို ဆူညံမှုဖယ်ပြီး ၎င်းတို့နုတ်လိုက်သောကျန်ရှိချက်ကို ထားရှိခြင်းဖြင့် အလုပ်လုပ်သည်။ ထို့ကြောင့် ဆူညံမှုဖယ်ပြီး ပုံကို ထားရှိခြင်းသည် လက်ဗွေ အခိုင်မာဆုံးဖြစ်သည့် ပုံ၏အပိုင်းတွင် အသုံးပြုသော အတိအကျ ပြောင်းပြန်လုပ်ဆောင်မှုဖြစ်သည်။\n\nပုံစံသည် အခန့်မှန်းရဆုံးဖြစ်ပြီး ထို့ကြောင့် တိုက်ခိုက်ရန် အထိုက်တန်ဆုံးဖြစ်သည့်နေရာ ဖြစ်၍ ၎င်းကို resolution အပြည့်တွင် ဦးစွာ လုပ်ဆောင်သည်။",
    },
    Section {
        heading: "ချုံ့ခြင်းက အဘယ်ကြောင့် အထောက်အကူ အများဆုံးဖြစ်သည်",
        body: "ဆက်စပ်မှုသည် ဓာတ်ပုံ၏ ပစ်ဆယ်တစ်ခုစီကို ကိုးကားပုံစံ၏ သက်ဆိုင်ရာအမှတ်နှင့် တန်းညှိခြင်းအပေါ် မူတည်သည်။ ချုံ့ခြင်းက ထိုသက်ဆိုင်မှုကို ဖျက်သည်။\n\nပုံကို အရွယ်ပြောင်းသည့်အခါ output ပစ်ဆယ်တစ်ခုစီကို input ပစ်ဆယ်များစွာမှ ရောစပ်သည်။ ပုံသေပုံစံကို အနီးအနားများနှင့် ပျမ်းမျှ၍ ဆင်ဆာနှင့် မကိုက်ညီတော့သော ကွက်အသစ်တစ်ခုပေါ်တွင် ပွားထုတ်သည်။ ဤနေရာရှိ လုပ်ဆောင်ချက်လေးခုအနက် ဤတစ်ခုက ဆက်စပ်မှုကို အများဆုံး လျှော့ချသည်။\n\nချုံ့ခြင်းက ပစ်ဆယ်ကွက်ကို နောက်ဆုံးထိသည့်အရာ ဖြစ်စေရန် ဆူညံမှုဖယ်ပြီးနောက် လုပ်ဆောင်သည်။",
    },
    Section {
        heading: "ဆူညံမှုအနည်းငယ် အဘယ်ကြောင့် ထည့်သည်",
        body: "ဆူညံမှုဖယ်ပြီး ချုံ့ပြီးနောက် ပုံစံ၏ ခြေရာအချို့ ကျန်နေသည်။ ကျပန်းဆူညံမှုအသစ်အနည်းငယ် ထည့်ခြင်းက ခွဲခြမ်းသူ ပုံမှ လုပ်နိုင်သော မည်သည့်ခန့်မှန်းချက်၏မဆို signal-to-noise အချိုးကို လျှော့ချသည်။\n\n၎င်းက ရှိသည့်အရာကို မဖျက်။ ရှိသည့်အရာကို တိုင်းတာရ ပိုခက်စေ၊ ၎င်းသည် စာရင်းအင်းစစ်ဆေးမှုအတွက် အနားစွန်းတွင် နီးပါးအတူတူပင်ဖြစ်သည်။",
    },
    Section {
        heading: "ပြန်ချုံ့ခြင်းက အဘယ်ကြောင့် အားအနည်းဆုံးအဆင့်ဖြစ်သည်",
        body: "အဖြစ်များသောအကြံဉာဏ်မှာ လက်ဗွေကို ဖျက်ဆီးရန် ဓာတ်ပုံကို ချုံ့ပြီး ပြန်ဖြေခြင်းဖြစ်သည်။ ၎င်းသည် ဤနေရာရှိ လုပ်ဆောင်ချက်များအနက် အထိရောက်နည်းဆုံးဖြစ်သည်။\n\nPRNU သည် အလယ်အလတ် JPEG ချုံ့မှုကို သက်တောင့်သက်သာ ကျော်ဖြတ်သည်။ ဆုံးရှုံးမှုပါချုံ့မှုက ကြိမ်နှုန်းမြင့်အသေးစိတ်အချို့ကို စွန့်ပစ်သဖြင့် အနည်းငယ် အထောက်အကူပြုသော်လည်း ၎င်းတစ်ခုတည်းဖြင့် လုံလောက်မည်မဟုတ်ခဲ့ပါ။ ၎င်းကို အားကိုးမည့်အစား နောက်ဆုံးအဆင့်အဖြစ် ထည့်သွင်းထားသည်။",
    },
    Section {
        heading: "အလုပ်မဖြစ်သည့်အရာ— အရောင်",
        body: "အဖြူ balance ကို ပြောင်း၊ အရောင်အရိပ်ထည့်၊ သို့မဟုတ် တစ်ခုချင်းစီ channel ၏ gain ကို ပြောင်းခြင်းက ဘာမျှ လုံးဝ မလုပ်။\n\nရှာဖွေကိရိယာများသည် normalized correlation ကို သုံးပြီး ၎င်းက နှိုင်းယှဉ်မီ ညီညာသော scaling သို့မဟုတ် offset ကို ဖယ်ထုတ်သည်။ ကမ္ဘာလုံးဆိုင်ရာအရောင်ပြောင်းလဲမှုသည် ထိုသို့သောပြောင်းလဲမှုအတိအကျဖြစ်သဖြင့် နှိုင်းယှဉ်မမီ သင်္ချာက ဖယ်ရှားသည်။ ၎င်းက အရောင်တိကျမှုကို ကုန်ကျစေပြီး ကာကွယ်မှု ဘာမျှ မဝယ်ပါ။\n\nဤသည်ကို ရေးထားရသည်မှာ ၎င်းသည် အလိုလိုနားလည်နိုင်သောအတွေးတစ်ခုဖြစ်ပြီး မှားနေ၍၊ အွန်လိုင်းရှိ အကြံဉာဏ်များစွာတွင် ပေါ်လာသောကြောင့်ဖြစ်သည်။",
    },
    Section {
        heading: "ရိုးသားသောကန့်သတ်ချက်",
        body: "ဤသည် ဆက်စပ်မှုကို လျှော့ချသည်။ ၎င်းက လက်ဗွေကို မဖယ်ရှား၊ ဤအက်ပ်ရှိ မည်သည့်အရာကမျှ ၎င်းဖယ်ရှားပြီဟု သင့်အား ဘယ်သောအခါမျှ မပြောပါ။\n\nခိုင်မာသောကိုးကားပုံစံ၊ နမူနာပုံများစွာနှင့် အချိန်ရှိသော forensic ခွဲခြမ်းသူတစ်ဦးက scaling factor များကို လျော်ကြေးပေးနိုင်ပြီး crop များကို ရှာဖွေနိုင်သည်။ ထိုအရာနှင့်ဆန့်ကျင်၍ ဤလုပ်ဆောင်ချက်များက ကိုက်ညီမှု၏ကုန်ကျစရိတ်ကို မြှင့်ပြီး ယုံကြည်မှုကို လျှော့ချသည်။ ၎င်းတို့က ကိုက်ညီမှုကို မဖြစ်နိုင်အောင် မလုပ်ပါ။\n\nသင့်ဘက်တွင်လည်း ကုန်ကျစရိတ်ရှိသည်။ ဤနေရာရှိ ဆက်တင်တိုင်းက ဓာတ်ပုံကို ယုတ်လျော့စေသည်— အသေးစိတ် ပိုနူးညံ့၊ ပစ်ဆယ် ပိုနည်း၊ ချုံ့မှု ပိုများ။ ထိုအလဲအလှယ်က သင့်ဆုံးဖြတ်ရန်ဖြစ်၍ သင်ဖွင့်မှသာ ၎င်း ဖွင့်ထားသည်။\n\nသင့်လုံခြုံရေးက ချိတ်ဆက်မခံရရန်အပေါ် မူတည်ပါက ပိုအားကောင်းသောနည်းလမ်းမှာ တူညီသောကင်မရာဖြင့် ကိုယ်ရေးအထောက်အထားနှစ်ခုအောက်တွင် အစကတည်းက မထုတ်ဝေခြင်းဖြစ်သည်။",
    },
];

const MYTHS_MY: &[Myth] = &[
    Myth {
        claim: "မက်ဆေ့ဂျ်အက်ပ်မှတစ်ဆင့် ဓာတ်ပုံပို့ခြင်းက အားလုံးကို ဖယ်ရှားသည်။",
        reality: "ပလက်ဖောင်းကြီးအများစုက ပုံတစ်ပုံကို ဓာတ်ပုံအဖြစ်ပို့သည့်အခါ EXIF ကို ဖယ်ပေးသဖြင့် တည်နေရာက များသောအားဖြင့် ပျောက်သွားသည်။ သတိထားစရာ နှစ်ချက်။ တူညီသောဖိုင်ကို စာရွက်စာတမ်း သို့မဟုတ် ဖိုင်တွဲအဖြစ်ပို့ခြင်းက မူရင်းကို မထိဘဲ မက်တာဒေတာအပါအဝင် အပ်လုဒ်တင်ပြီး လူတို့က အရည်အသွေးပိုကောင်းရန် ထိုရွေးချယ်မှုကို ရွေးကြရာ ၎င်းနှင့်အတူ ဘာလိုက်ပါလာသည်ကို မသိကြပါ။ WhatsApp နှင့် Telegram နှစ်ခုစလုံး ဤသို့ ပြုမူသည်၊ Signal က စာရွက်စာတမ်း mode တွင်ပါ မက်တာဒေတာဖယ်သည်—၎င်းတို့အကြား တကယ့်ကွာခြားချက်ဖြစ်သည်။ ၎င်းတို့တစ်ခုမျှ ပစ်ဆယ်များထဲရှိ ဆင်ဆာပုံစံကို မထိ၊ အဘယ်ကြောင့်ဆိုသော် ၎င်းသည် မက်တာဒေတာ မဟုတ်၍။",
    },
    Myth {
        claim: "screenshot ရိုက်ခြင်းက မက်တာဒေတာကို ဖယ်ရှားသည်။",
        reality: "အများအားဖြင့် မှန်ပြီး ၎င်းက ဆင်ဆာပုံစံကိုလည်း ရှုပ်ထွေးစေသည်၊ အဘယ်ကြောင့်ဆိုသော် ဆင်ဆာမှတ်တမ်းတင်သည့်အရာထက် ဖန်သားပြင်ပြသသည့်အရာကို ဖမ်းယူသောကြောင့်။ သို့သော် screenshot က ကိုယ်ပိုင်မက်တာဒေတာအသစ်ကို သယ်ဆောင်ပြီး အရည်အသွေးမှာ သင့်တော်သောသန့်စင်မိတ္တူထက် များစွာဆိုးကာ၊ အဖြစ်များသောအမှားမှာ လုံခြုံရေးအတွက် screenshot ရိုက်ပြီးမှ မူရင်းကို မတော်တဆ ပို့မိခြင်းဖြစ်သည်။",
    },
    Myth {
        claim: "ဖိုင်ကို အမည်ပြောင်းခြင်း သို့မဟုတ် zip ထဲထည့်ခြင်းက မက်တာဒေတာကို ဖယ်ရှားသည်။",
        reality: "နှစ်ခုစလုံး ဘာမျှ လုံးဝ မလုပ်။ ဖိုင်အမည်သည် ဖိုင်၏အကြောင်းအရာ၏ အစိတ်အပိုင်းမဟုတ်ပြီး archive က ဖိုင်ကို အတိအကျ ထိန်းသိမ်းသဖြင့် တစ်ဖက်မှ မပြောင်းဘဲ ထွက်လာသည်။ ၎င်းသည် archive ၏ အဓိကရည်ရွယ်ချက်ပင်ဖြစ်သည်။",
    },
    Myth {
        claim: "ငါ့ဓာတ်ပုံ မတည်းဖြတ်ရသေးလို့ raw ဖိုင်ဖြစ်တယ်။",
        reality: "သင်မထိရသေးသောဓာတ်ပုံသည် raw ဖိုင် မဟုတ်သေးပါ။ ဖုန်း သို့မဟုတ် ကင်မရာမှ တိုက်ရိုက်ရသော JPEG ကို ကင်မရာအတွင်း develop ပြီးသား— အရောင်၊ ကွန်ထရပ်နှင့် ချွန်ထက်မှုထည့်၊ ပြီးမှ ချုံ့ပြီးဖြစ်သည်။ raw သည် ထိုအရာအားလုံးမတိုင်မီ ဆင်ဆာဒေတာဖြစ်ပြီး ဖွင့်ရန် အထူးဆော့ဖ်ဝဲလိုသော ထုတ်လုပ်သူသီးသန့်ဖော်မတ် (CR2, CR3, NEF, ARW, DNG, RW2, ORF, RAF နှင့် အခြားများ) ဖြင့် ရှိသည်။ ၎င်းကို ဆုံးဖြတ်သည်မှာ ဖိုင်အမျိုးအစားဖြစ်ပြီး ပုံကို တည်းဖြတ်ခဲ့ခြင်း ရှိမရှိ မဟုတ်။ ဤနေရာတွင် အရေးကြီးသည်မှာ raw က ဖယ်ရှားရန်ပိုသယ်ဆောင်ပြီး ဤကိရိယာက raw ကို တတ်နိုင်သမျှသာ သန့်စင်နိုင်၍ ပြန်မတည်ဆောက်နိုင်သောကြောင့်။ «ကင်မရာ raw ဖိုင်များ» ကို ကြည့်ပါ။",
    },
    Myth {
        claim: "PNG သို့ ပြောင်းခြင်းက မက်တာဒေတာကို ဖယ်ရှားသည်။",
        reality: "PNG တွင် ပြည့်ဝသော EXIF အပိုင်းအစနှင့် လွတ်လပ်သောစာသားအကွက်များအပါအဝင် ကိုယ်ပိုင်မက်တာဒေတာအပိုင်းအစများရှိပြီး converter အများက tag များကို စွန့်ပစ်မည့်အစား ကူးယူတတ်သည်။ ဖော်မတ်ပြောင်းခြင်းသည် အချက်အလက်ဖယ်ရှားခြင်းနှင့် မတူပါ။",
    },
    Myth {
        claim: "တည်နေရာဝန်ဆောင်မှုပိတ်ထားလို့ ငါ့ဓာတ်ပုံတွေ အဆင်ပြေတယ်။",
        reality: "၎င်းက အကြီးဆုံးတစ်ခုတည်းသောအရာဖြစ်သည့် GPS တည်နေရာကို ဖယ်ရှားပြီး လုပ်ထိုက်သည်။ ကျန်အားလုံး ကျန်နေသည်— ကင်မရာမော်ဒယ်၊ လင့်စ်၊ maker note ရှိအမှတ်စဉ်နံပါတ်၊ တိကျသောအချိန်တံဆိပ်၊ ထည့်သွင်းပုံသေး၊ နှင့် တည်းဖြတ်မှတ်တမ်း။",
    },
    Myth {
        claim: "ဓာတ်ပုံကို ချုံ့လိုက်ရုံနဲ့ ကင်မရာလက်ဗွေ ပျောက်သွားတယ်။",
        reality: "ဤသည်မှာ အဖြစ်များဆုံးအကြံဉာဏ်ဖြစ်ပြီး အသုံးဝင်သောလုပ်ဆောင်ချက်များအနက် အားအနည်းဆုံးဖြစ်သည်။ ပုံစံသည် အလယ်အလတ် JPEG ချုံ့မှုကို သက်တောင့်သက်သာ ကျော်ဖြတ်သည်။ ချုံ့ခြင်းက အနားစွန်းတွင် အထောက်အကူပြု၊ အရွယ်ပြောင်းခြင်းနှင့် ဆူညံမှုဖယ်ခြင်းက တကယ့်အလုပ်ကို လုပ်သည်။",
    },
    Myth {
        claim: "အရောင် သို့မဟုတ် အဖြူ balance ကိုပြောင်းလိုက်ရင် လက်ဗွေကို အနိုင်ယူတယ်။",
        reality: "၎င်း ဘာမျှ မလုပ်။ နှိုင်းယှဉ်မှုသည် normalized correlation ဖြစ်ပြီး နှစ်ခုကို နှိုင်းယှဉ်မီ တစ်ခုချင်း channel scaling သို့မဟုတ် offset ကို ဖယ်ထုတ်သည်။ ကမ္ဘာလုံးဆိုင်ရာအရောင်ပြောင်းလဲမှုကို အကျိုးသက်ရောက်မှုမရှိမီ သင်္ချာက ဖယ်ရှားသည်။",
    },
    Myth {
        claim: "ဓာတ်ပုံကို ဖြတ်တောက်လိုက်ရင် လက်ဗွေကို အနိုင်ယူတယ်။",
        reality: "၎င်းက အထောက်အကူပြုသည်၊ အဘယ်ကြောင့်ဆိုသော် နှိုင်းယှဉ်မှုမူတည်သည့် တန်းညှိမှုကို ရွှေ့သောကြောင့်၊ သို့သော် ခွဲခြမ်းသူက ဖြစ်နိုင်သော crop အနေအထားများကို ရှာဖွေနိုင်သည်။ ၎င်းက ကျန်ရှိသောပစ်ဆယ်များတွင်လည်း ပုံစံကို မထိဘဲ ချန်ထားသည်။",
    },
    Myth {
        claim: "ဆင်ဆာလက်ဗွေယူခြင်းဟာ သီအိုရီ ဒါမှမဟုတ် ရုပ်ရှင်ထဲကအရာ။",
        reality: "၎င်းသည် 2006 ခုနှစ်အထိ ပြန်သွားသော သုတေသနစာပေရှိ၊ တကယ့်အမှုများတွင် အသုံးပြုသော မှတ်တမ်းတင်ထားသောနည်းစနစ်ဖြစ်သည်။ ၎င်းကို စိတ်ကူးယဉ်အဖြစ် သဘောထားခြင်းသည် ၎င်းကို အမှားကင်းသည်ဟု သဘောထားခြင်းလောက်ပင် အမှားဖြစ်သည်။",
    },
    Myth {
        claim: "ဆင်ဆာလက်ဗွေယူခြင်းက သင့်ကို အမြဲ ဖော်ထုတ်နိုင်သည်ဟု ဆိုလိုသည်။",
        reality: "အခြားတစ်ဖက်တွင်လည်း တူညီစွာ မှားသည်။ ကိုက်ညီရန် သင့်ကင်မရာအထူးအတွက် ကိုးကားပုံစံ လိုအပ်ပြီး ၎င်းကို ရုပ်ပိုင်းဆိုင်ရာစက်မှ သို့မဟုတ် သင့်အဖြစ် သိရှိပြီးသောဓာတ်ပုံများမှ တည်ဆောက်သည်။ ၎င်းမရှိဘဲ နှိုင်းယှဉ်စရာ ဘာမျှ မရှိ။ ၎င်းက ဓာတ်ပုံများကို အချင်းချင်း ချိတ်ဆက်သည်၊ အမည်တစ်ခု မထုတ်ပေး။",
    },
    Myth {
        claim: "ဖိုင် သန့်ပြီ၊ ဒါကြောင့် ငါ့ဆီ ပြန်ခြေရာမခံနိုင်ဘူး။",
        reality: "ဖိုင်ကို သန့်စင်ခြင်းက ဖိုင်အတွင်းရှိအရာကို ကိုင်တွယ်သည်။ ဝင်ရောက်ထားစဉ် ၎င်းကို အပ်လုဒ်တင်ခဲ့ပါက ပလက်ဖောင်းတွင် မည်သည့်အကောင့်က ဘာကို ဘယ်အချိန် ပို့သည်ဟူသော ကိုယ်ပိုင်မှတ်တမ်းရှိပြီး ဖိုင်မည်မျှ သန့်စေ ထိုမှတ်တမ်းကို ဥပဒေအရ ရယူနိုင်သည်။ အချို့ပလက်ဖောင်းများက လမ်းတစ်လျှောက် ကိုယ်ပိုင်အမှတ်အသားကိုပါ ပုံများထဲသို့ ရေးသည်။ «ဤကိရိယာ မလှမ်းမီသည့်အရာ» ကို ကြည့်ပါ။",
    },
    Myth {
        claim: "ချုံ့ပြီး ပြန်ချုံ့ခြင်းက ပလက်ဖောင်းက ပုံကို မှတ်မိခြင်းကို တားဆီးသည်။",
        reality: "မဟုတ်ပါ။ ပလက်ဖောင်းများသည် ပုံများကို perceptual hash များဖြင့် ကိုက်ညီစေပြီး ၎င်းတို့ကို အရွယ်ပြောင်းခြင်း၊ ပြန်ချုံ့ခြင်းနှင့် အသေးစားတည်းဖြတ်မှုများကို ကျော်ဖြတ်ရန် အထူးတည်ဆောက်ထားသည်။ ဤအက်ပ်ရှိ လက်ဗွေလျှော့ချမှုက ဆင်ဆာပုံစံကို ရှုပ်ထွေးစေသည်၊ ဖိုင်နှစ်ခုသည် တူညီသောဓာတ်ပုံဖြစ်ကြောင်း ပလက်ဖောင်းမြင်ခြင်းကို မတားဆီး။ တိုက်ခိုက်မှုကွဲ၊ ကာကွယ်မှုကွဲ။",
    },
    Myth {
        claim: "ဒါက သတင်းထောက် ဒါမှမဟုတ် တက်ကြွလှုပ်ရှားသူဆိုမှ အရေးကြီးတယ်။",
        reality: "အဖြစ်များဆုံးသော တကယ့်အန္တရာယ်မှာ အိမ်တွင်းဆိုင်ရာဖြစ်သည်။ အများသိ တင်ထားသောဓာတ်ပုံတစ်ခုသည် ၎င်းရိုက်ခဲ့သည့်နေရာ၏ တည်နေရာကို သယ်ဆောင်နိုင်ပြီး၊ ပုံသေးတစ်ခုသည် အကြောင်းပြချက်တစ်ခုကြောင့် ဖြတ်တောက်ခဲ့သောဗားရှင်းကို သယ်ဆောင်နိုင်သည်။",
    },
];

const EVIDENCE_MY: &[Section] = &[
    Section {
        heading: "နည်းစနစ်က ဘယ်ကလာသလဲ",
        body: "ဆင်ဆာလက်ဗွေယူခြင်းကို Lukáš, Fridrich နှင့် Goljan တို့က 2006 ခုနှစ် IEEE Transactions on Information Forensics and Security တွင်ထုတ်ဝေသော «Digital Camera Identification from Sensor Pattern Noise» တွင် တည်ထောင်ခဲ့သည်။ ၎င်းသည် အခြေခံစာတမ်းဖြစ်ပြီး ဤနယ်ပယ်၏အခြေခံအဖြစ် ဆက်လက်ရှိနေသည်။\n\nသူတို့၏နည်း— ကင်မရာတစ်လုံးဖြင့် ဓာတ်ပုံများစွာ ရိုက်၊ တစ်ခုစီကို ဆူညံမှုဖယ်၊ ကျန်ရှိချက်ကို ထားရှိ၊ ပုံသေအစိတ်အပိုင်း ပိုအားကောင်းလာစေရန်နှင့် ကျပန်းအစိတ်အပိုင်း ပျက်ကွယ်စေရန် ထိုကျန်ရှိချက်များကို ပျမ်းမျှခြင်းဖြင့် ကင်မရာအတွက် ကိုးကားပုံစံကို တည်ဆောက်သည်။ ပြီးမှ မေးခွန်းရှိဓာတ်ပုံကို တူညီသောနည်းဖြင့် ဆူညံမှုဖယ်၍ ၎င်း၏ကျန်ရှိချက်ကို ထိုကိုးကားနှင့် ဆက်စပ်သည်။",
    },
    Section {
        heading: "ကိုးကားပုံစံက အဓိကဇာတ်လမ်းတစ်ခုလုံးဖြစ်ရသည့်အကြောင်း",
        body: "ကိုးကားကို ကင်မရာတစ်လုံးမှ ပုံများစွာကို ပျမ်းမျှ၍ တည်ဆောက်သဖြင့် ခွဲခြမ်းသူတွင် စက် သို့မဟုတ် ၎င်းသို့ ချိတ်ဆက်ထားသောဓာတ်ပုံအစုတစ်ခု ရှိပြီးသားဖြစ်ရမည်။\n\nဤသည် ခြိမ်းခြောက်မှုအကြောင်း အရေးအကြီးဆုံးအချက်ဖြစ်ပြီး အများဆုံး ချန်လှပ်ခံရသောအချက်ဖြစ်သည်။ နည်းစနစ်က «ဤအရာများ တူညီသောဆင်ဆာမှ လာသလား?» ကို ဖြေသည်။ တစ်စုံတစ်ဦးက အဖြေကို ပေးပြီးမှသာ တစ်ပါး ၎င်းက «ဤသည် ဘယ်သူ့ကင်မရာလဲ?» ကို မဖြေပါ။",
    },
    Section {
        heading: "အရွယ်ပြောင်းခြင်းအကြောင်း သက်သေက ဘာပြောသလဲ",
        body: "ဤနေရာတွင် ရိုးသားမှုက အရေးအကြီးဆုံးဖြစ်သည်၊ အဘယ်ကြောင့်ဆိုသော် အရွယ်ပြောင်းခြင်းသည် ဤကိရိယာလုပ်သည့် အဓိကအရာဖြစ်၍။\n\nစာပေက တညီတညွတ်တည်း— ချုံ့ထားသောပုံများမှ ဖော်ထုတ်ခြင်း ဖြစ်နိုင်ဆဲရှိသော်လည်း စွမ်းဆောင်ရည် သိသိသာသာ ကျဆင်းသည်။ အရွယ်ပြောင်းခြင်းသည် low-pass filter အဖြစ်လုပ်ဆောင်ပြီး scale factor ကွဲပြားချက်များက signal ၏ကွဲပြားသောအပိုင်းများကို ထိန်းသိမ်းသည်။ scale factor ကို သိ သို့မဟုတ် မှန်းဆနိုင်သောခွဲခြမ်းသူက ၎င်းကို လျော်ကြေးပေးနိုင်သည်။\n\nထို့ကြောင့် ချုံ့ခြင်းသည် ဤနေရာရှိ အထိရောက်ဆုံးလုပ်ဆောင်ချက်ဖြစ်ပြီး ၎င်းသည်လည်း အရှုံးမဟုတ်သေး။ «သိသိသာသာ ကျဆင်းစေသည်» သည် ရိုးသားသောဖော်ပြချက်ဖြစ်ပြီး ဤအက်ပ်က ၎င်းကို သုံးသည်။",
    },
    Section {
        heading: "counter-forensics အကြောင်း သက်သေက ဘာပြောသလဲ",
        body: "ဆင်ဆာလက်ဗွေများကို ဆန့်ကျင်သော counter-forensic နည်းများသည် ဖြေရှင်းပြီးပြဿနာထက် တက်ကြွသောသုတေသနနယ်ပယ်ဖြစ်သည်။ ထုတ်ဝေထားသောနည်းများတွင် ပစ်ဆယ်တန်ဖိုးများ ဖြစ်နိုင်ဖွယ်ရှိသော်လည်း မူရင်းကွက်နှင့် မကိုက်ညီတော့စေရန် interpolation နည်းတစ်ခုဖြင့် ချဲ့၍ အခြားတစ်ခုဖြင့် ချုံ့ခြင်း၊ နှင့် ဆူညံမှုဖိနှိပ်ခြင်းနှင့် ထည့်သွင်းခြင်းပုံစံများစွာ ပါဝင်သည်။\n\nထိုစာပေတွင် တစ်ခုမျှ အာမခံချက်အဖြစ် မတင်ပြထားပါ။ ၎င်းတို့ကို attribution ကို ပိုခက်ခဲစေသည်ဟု ဖော်ပြထားပြီး—၎င်းသည် မတူညီသောဆိုချက်ဖြစ်ကာ ဤနေရာတွင် ပြုသောဆိုချက်ဖြစ်သည်။",
    },
    Section {
        heading: "သုတေသီများ သဘောကွဲသည့်နေရာ",
        body: "တကယ့်အခြေအနေအောက်တွင် ယုံကြည်စိတ်ချရမှုကို ဆွေးနွေးနေဆဲဖြစ်သည်။ မကြာသေးမီအလုပ်များက ဖိုင်မရေးမီကတည်းက လေးလံသောကွန်ပျူတာလုပ်ဆောင်မှု၊ ပြင်းထန်သောဆူညံမှုလျှော့ချမှုနှင့် ဒစ်ဂျစ်တယ်တည်ငြိမ်စေမှုအားလုံးက ပုံစံကို ဝင်ရောက်စွက်ဖက်သည့် ခေတ်မီစမတ်ဖုန်းများပေါ်တွင် နည်းစနစ် မည်မျှကောင်းစွာ ခံနိုင်သည်ကို မေးမြန်းသည်။\n\nနယ်ပယ်တွင် အမှုစစ်အတွက် အတည်ဖြစ်သောစံ ရှိမရှိအကြောင်း ဆက်လက်ဆွေးနွေးမှုလည်း ရှိသည်။ အဖြေက ရိုးရှင်းသည်ဟု မည်သည့်ဘက်ဖြစ်စေ သင့်အား ပြောသူတိုင်းသည် သက်သေထက် ရှေ့ရောက်နေသည်။",
    },
    Section {
        heading: "ဤသည် သင့်အတွက် ဘာကို ဆိုလိုသလဲ",
        body: "မက်တာဒေတာဖယ်ရှားခြင်းသည် သက်သေပြနိုင်သောအပိုင်းဖြစ်သည်။ အချက်အလက်သည် သတ်မှတ်နေရာများတွင်ရှိ၊ ဖယ်ရှားခံရ၊ ရလဒ်ကို အခြားကိရိယာဖြင့် စစ်ဆေးနိုင်သည်။\n\nဆင်ဆာလက်ဗွေလျှော့ချခြင်းသည် စာရင်းအင်းဆိုင်ရာဖြစ်သည်။ ၎င်းက ကိုက်ညီမှု၏ယုံကြည်မှုကို သင့်အထူးဓာတ်ပုံ၊ ကင်မရာနှင့် ရန်သူအတွက် မည်သူမျှ တိကျစွာ မပြောနိုင်သောပမာဏဖြင့် လျှော့ချသည်။\n\nထိုအရာများသည် မတူညီသောဆိုချက်အမျိုးအစားများဖြစ်၍ ဤအက်ပ်က ၎င်းတို့ကို ထိုအကြောင်းကြောင့် မြင်သာစွာ ခွဲထားသည်။ သင့်လုံခြုံရေးက ချိတ်ဆက်မခံရရန်အပေါ် မူတည်ပါက လက်ဗွေအလုပ်ကို ၎င်းကို ဖြေရှင်းသည့်အရာအဖြစ်မဟုတ်ဘဲ အလွှာများစွာအနက် တစ်လွှာအဖြစ် သဘောထားပါ။",
    },
];

const BEYOND_THE_FILE_MY: &[Section] = &[
    Section {
        heading: "သန့်သောဖိုင်သည် အမည်ဝှက်အပ်လုဒ်မဟုတ်",
        body: "ဤအကန့်ရှိ ကျန်အားလုံးသည် ဖိုင်အတွင်းသယ်ဆောင်သောအချက်အလက်အကြောင်းဖြစ်သည်။ ဤအပိုင်းသည် တူညီသောဖိုင်ကို ဖော်ပြသည့် အခြားနေရာတစ်ခုတွင် ရှိသောအချက်အလက်အကြောင်းဖြစ်သည်။\n\nဝင်ရောက်ထားစဉ် ဓာတ်ပုံတစ်ပုံ အပ်လုဒ်တင်ပါက ဖိုင်မည်မျှ သန့်စေ မည်သည့်အကောင့်က ဘယ်အချိန်၊ ဘယ်လိပ်စာမှ တင်သည်ကို ပလက်ဖောင်းက သိသည်။ မက်တာဒေတာဖယ်ရှားခြင်းက အပ်လုဒ်မှတ်တမ်းကို မဖယ်ရှား။ ၎င်းသည် ဖိုင်ထဲ အစကတည်းက မရှိခဲ့ပါ။\n\nဤကိရိယာက ထိုအရာမှန်သမျှသို့ မလှမ်းမီနိုင်၊ ဤအမျိုးအစား မည်သည့်ကိရိယာမျှ မလှမ်းမီနိုင်။",
    },
    Section {
        heading: "ပလက်ဖောင်းများက ကိုယ်ပိုင်အမှတ်အသားများ ထည့်သည်",
        body: "အချို့ဝန်ဆောင်မှုများသည် မက်တာဒေတာကို ဖယ်ရှားရုံသာမက ကိုယ်ပိုင်ကို ရေးသွင်းသည်။\n\nFacebook သည် 2014 ခန့်ကတည်းက အပ်လုဒ်တင်ပုံများ၏ IPTC အပိုင်းတွင် ထုတ်လွှင့်ကိုးကားရန်ရည်ရွယ်သောအကွက်၌ «FBMD» ဖြင့်စသောတန်ဖိုးများဖြင့် အမှတ်အသားတစ်ခုကို ထည့်သွင်းခဲ့သည်။ ၎င်းကို လုံခြုံရေးသုတေသီတစ်ဦးက 2019 တွင် တွေ့ရှိပြီး၊ သုံးထားသောစံကို ပိုင်ဆိုင်သော IPTC က ထိုအလေ့အထ၏ စာရွက်စာတမ်းကို ရှာ၍ မတွေ့ခဲ့ပါ။\n\nလက်တွေ့အကျိုးသက်ရောက်မှု— ပလက်ဖောင်းမှ ဒေါင်းလုဒ်လုပ်ထားသောပုံသည် အခြားနည်းဖြင့် ဖယ်ရှားပြီးဟု ထင်ရသောဖိုင်ပေါ်တွင် ပလက်ဖောင်းက အနက်ဖွင့်နိုင်သောအမှတ်အသားကို သယ်ဆောင်နိုင်သည်။ ဤအက်ပ်က IPTC အပိုင်းများကို လုံးဝ ဖယ်ရှားသဖြင့် ဒေါင်းလုဒ်လုပ်ထားသောပုံကို ၎င်းမှတစ်ဆင့် ဖြတ်သန်းစေခြင်းက ထိုအမှတ်အသားကိုပါ ဖယ်ရှားသည်။ ၎င်းမလုပ်နိုင်သည်မှာ ပလက်ဖောင်းသိမ်းထားသောမိတ္တူကို ဖယ်ရှားခြင်းဖြစ်သည်။",
    },
    Section {
        heading: "ဥပဒေလုပ်ငန်းစဉ်က ဘာရနိုင်သလဲ",
        body: "တိကျသောယန္တရားကို သိထိုက်သည်၊ အဘယ်ကြောင့်ဆိုသော် ၎င်းကို မကြာခဏ လွန်စွာ ပေါ့ဆစွာ ဖော်ပြတတ်၍။\n\nMeta ၏ ထုတ်ဝေထားသောလမ်းညွှန်ချက်များအရ ရာဇဝတ်စုံစမ်းစစ်ဆေးမှုတွင် subpoena တစ်ခုက အခြေခံ subscriber မှတ်တမ်းများ—အမည်၊ ဝန်ဆောင်မှုကာလ၊ အီးမေးလ်လိပ်စာများနှင့် မကြာသေးမီ login လိပ်စာများ—ကို တောင်းဆိုသည်။ မက်ဆေ့ဂျ်၊ ဓာတ်ပုံနှင့် ဗီဒီယိုများပါဝင်သော အကောင့်၏သိမ်းဆည်းအကြောင်းအရာကို တောင်းဆိုရန် probable cause ပြသ၍ search warrant လိုအပ်သည်။ အကြောင်းအရာသည် subscriber အသေးစိတ်ထက် ပိုမြင့်သောအဆင့်ဖြစ်ပြီး တူညီသည်မဟုတ်။\n\nသိမ်းဆည်းမှုတွင် အချိန်အစိတ်အပိုင်းလည်း ရှိသည်။ Meta သည် ဥပဒေလုပ်ငန်းစဉ်ကို စောင့်လျက် မှတ်တမ်းများကို ထိန်းသိမ်းသော်လည်း ထိန်းသိမ်းရန်တောင်းဆိုချက်သည် ပစ္စည်းမဖျက်မီ ရောက်ရမည်။ ပျောက်သွားပြီးဒေတာသည် ပျောက်ပြီ။\n\nထို့ကြောင့် «တစ်စုံတစ်ဦးက ဤဖိုင်ကို ပို့သူနှင့် ပြန်ကိုက်ညီစေနိုင်သလား» ဆိုသည့်အဖြေမှာ— မှန်ကန်သောဥပဒေလုပ်ငန်းစဉ်ဖြင့်၊ မှန်ကန်သောတရားစီရင်ပိုင်ခွင့်နယ်တွင်၊ ထိန်းသိမ်းကာလအတွင်း—မကြာခဏ ဟုတ်သည်။ ၎င်းသည် ဖိုင်၏မက်တာဒေတာနှင့် မသက်ဆိုင်ပါ။",
    },
    Section {
        heading: "Perceptual hashing နှင့် ဤကိရိယာ၏ ကန့်သတ်ချက်တစ်ခု",
        body: "ပလက်ဖောင်းကြီးများသည် ပုံများကို perceptual hash များဖြင့် ကိုက်ညီစေပြီး ၎င်းတို့ကို သာမန် checksum ကို ဖျက်သည့်ပြောင်းလဲမှုများ—အရွယ်ပြောင်း၊ ပြန်ချုံ့၊ အသေးစားအရောင်ရွှေ့မှုများ၊ အသေးစား crop များ—ကို အတိအကျ ကျော်ဖြတ်ရန် ဒီဇိုင်းဆွဲထားသည်။\n\nဤသည် ဤနေရာတွင် ပေးသော လက်ဗွေလျှော့ချမှုအတွက် တိုက်ရိုက်အကျိုးဆက်ရှိသည်။ ဆူညံမှုဖယ်၊ ချုံ့နှင့် ပြန်ကုဒ်ပြုခြင်းသည် ဆင်ဆာပုံစံကို ရှုပ်ထွေးစေရန်ရည်ရွယ်ပြီး perceptual hashing ကို ထိုလုပ်ဆောင်ချက်များအပေါ် ဂရုမစိုက်စေရန် တည်ဆောက်ထားသည်။ သန့်စင်ပြီးမိတ္တူသည် ထိုနှိုင်းယှဉ်မှုအောက်တွင် ၎င်း၏မူရင်းနှင့် ကိုက်ညီနေဆဲဖြစ်မည်။\n\nဤသည် မတူညီသောကာကွယ်မှုများပါသော မတူညီသောတိုက်ခိုက်မှုများဖြစ်သည်။ ဤအက်ပ်ရှိ မည်သည့်အရာကမျှ ပုံနှစ်ခုသည် တူညီသောဓာတ်ပုံဖြစ်ကြောင်း ပလက်ဖောင်းမှတ်မိခြင်းကို ကာကွယ်ခြင်း မဟုတ်။",
    },
    Section {
        heading: "ကျန်မိတ္တူတိုင်း",
        body: "သင်သန့်စင်သောဖိုင်သည် မိတ္တူတစ်ခုဖြစ်သည်။ မူရင်းသည် သင့်ဓာတ်ပုံ roll တွင် ရှိနေဆဲ၊ cloud အရန်သိမ်းတွင် ဖြစ်နိုင်ခြေများ၊ မက်ဆေ့ဂျ်အက်ပ်၏ကိုယ်ပိုင် cache တွင် ဖြစ်နိုင်ပြီး၊ သင်ပို့ပြီးနောက် သင်ဘာမျှ ဆုံးဖြတ်ခွင့်မရှိသော အခြားသူ၏စက်ပေါ်တွင်ဖြစ်သည်။\n\nပို့မီ မိတ္တူတစ်ခုကို သန့်စင်ခြင်း လုပ်ထိုက်သည်။ ၎င်းသည် အချက်အလက် ရပ်တန့်သွားခြင်းနှင့် မတူပါ။",
    },
    Section {
        heading: "တကယ် အထောက်အကူပြုသည့်အရာ",
        body: "စိုးရိမ်မှုက ဖိုင်ဖြစ်ပါက ဖိုင်ကို သန့်စင်ပါ။ ၎င်းသည် ဤကိရိယာ၏ရည်ရွယ်ချက်ဖြစ်ပြီး ၎င်းကို ကောင်းစွာ လုပ်ဆောင်သည်။\n\nစိုးရိမ်မှုက အပ်လုဒ်တစ်ခုကို သင့်ဆီ ခြေရာမခံနိုင်သင့်ခြင်းဖြစ်ပါက ဖိုင်သည် အသေးဆုံးဖြစ်သည်။ သင်သုံးသောအကောင့်၊ ၎င်းကိုပြုလုပ်ခဲ့သည့်ဆက်သွယ်မှု၊ အကောင့်နောက်ကွယ်ရှိ ငွေပေးချေနည်းနှင့် ၎င်းလာသောစက်—အားလုံးက ပိုအရေးကြီးပြီး ၎င်းတို့တစ်ခုမျှ ဤနေရာတွင် မဖြေရှင်းပါ။\n\nထိုနယ်နိမိတ်အကြောင်း ရှင်းလင်းခြင်းက အားလုံးကို ဖုံးအုပ်ပြီးဟု အရိပ်အမြွက်ပြသောကိရိယာထက် ပိုအသုံးဝင်သည်။",
    },
];

// ===========================================================================
// Latin (draft). Machine translation for the Living-Latin / classicist
// community to refine. Technical tokens kept in Latin/original on purpose.
// ===========================================================================

const METADATA_LA: &[Item] = &[
    Item {
        name: "EXIF",
        what: "Fragmentum notarum a photomachina scriptum: exemplar, lens, numerus serialis, numerus actionum obturamenti, expositionis optiones, dies et hora ad secundam usque, et saepe GPS-coordinatae.",
        why: "GPS est illud manifestum, et ad paucos metros exactum est, ita ut una feriarum photographia inscriptionem domus prodere possit. Cetera quietiora sunt sed accumulantur: idem photomachinae corpus eademque lens per photographiarum copiam eas ad unum hominem ligant, etiam cum nihil aliud id facit.",
    },
    Item {
        name: "Nota fabricatoris (maker note)",
        what: "Regio privata venditoris intra EXIF, in forma non documentata quae inter fabricatores et firmware versiones differt.",
        why: "Hic plerumque numerus serialis sensoris habitat, una cum numero actionum obturamenti et optionibus internis. Est campus maxime identificans in photographia typica, et quia forma privata est, instrumenta quae solas notas standardas intellegunt eam saepe in loco relinquunt.",
    },
    Item {
        name: "Imago minuta (thumbnail)",
        what: "Parvum exemplar imaginis, intra documentum servatum ut spectatores praevisionem monstrare possint sine tota imagine decodenda.",
        why: "Imagines minutae semel generantur et saepe post retractationem non regenerantur. Photographia recisa ut aliquem e margine tollat potest adhuc originale non recisum intra se ferre. Idem valet de faciebus obscuratis et particulis superpictis.",
    },
    Item {
        name: "XMP",
        what: "Forma metadatorum Adobe, ut XML servata. A programmatibus retractandi et ab aliquibus photomachinis scribitur.",
        why: "Fert historiam retractationis, programma et versionem adhibitam, aestimationes, verba clavis, et saepe nomen auctoris aut possessoris licentiae. Hic etiam identificatores catalogi habitant, qui imaginem divulgatam ad certam bibliothecam in certa machina referre possunt.",
    },
    Item {
        name: "IPTC",
        what: "Fragmentum metadatorum prensae et editionis, saepe a programmatibus administrandi photographias scriptum.",
        why: "Destinatum ad nomen auctoris, notam iuris auctoris, contactus particulas et textum tituli ferendum. Omnia utilia officio nuntiorum, omnia identificantia cuivis qui sub suo nomine edere non intendebat.",
    },
    Item {
        name: "Descriptio coloris (ICC)",
        what: "Descriptio ICC quae explicat quomodo colorum numeri documenti interpretandi sint.",
        why: "Plerumque innoxia, et servata si rogas. Ex more removetur duabus de causis: descriptio textum liberum et exemplar instrumenti fert, et descriptio propria ex monitore calibrato satis singularis esse potest ut documenta liget. Si imagines tuae lati coloris sunt et colorum accuratio refert, eam iterum accende.",
    },
    Item {
        name: "Data adiuncta",
        what: "Quidquid post punctum ubi forma imaginis documentum finiri dicit adiungitur.",
        why: "Pleraque instrumenta legere desinunt ad notam finis, quod spatium post eam bonum latibulum facit. Aliquae photomachinae telephonicae ibi secundam photographiam plenae resolutionis servant. Quidquid in eo est, non est pars imaginis et cum documento iter facit.",
    },
    Item {
        name: "Informatio documenti",
        what: "In PDF et documentis Office: titulus, argumentum, auctor, programma quod id produxit, et tempora creationis et mutationis.",
        why: "Campus auctoris saepe nomen verum aut nomen usoris corporati est, automatice ex ratione sub qua programma installatum est impletus, numquam ei qui scribit ostensus.",
    },
    Item {
        name: "Identificatores retractationum",
        what: "Identificatores fortuiti quos programmata verba tractantia in documentum scribunt et dum retractatur renovant.",
        why: "Duo documenta idem identificatorem retractationis communicantia in eadem sessione in eadem machina retractata sunt. Hoc documenta ligat quae nihil aliud commune habent, et fere nemo scit hunc campum exsistere.",
    },
    Item {
        name: "Structurae non agnitae",
        what: "Quilibet fragmentum in continente quod in indice servandorum non est, sectionibus privatis venditoris quas hoc instrumentum numquam vidit inclusis.",
        why: "Causa cur instrumentum documenta reficiat potius quam retractet. Instrumentum quod metadata quae agnoscit delet quidquid novum, privatum, aut consulto occultatum est tacite transire sinet. Ex indice servandorum reficere significat morem pro re ignota esse eam abicere.",
    },
];

const FILE_TYPES_LA: &[FileType] = &[
    FileType {
        name: "Photographia JPEG (.jpg)",
        carries: "Fragmentum EXIF a photomachina aut telephono scriptum: GPS-coordinatae, photomachinae nota, exemplar et numerus serialis, lens, dies et hora exacta ad secundam, et parva imago minuta. Saepe etiam fragmentum XMP ex programmate retractandi.",
        identifies: "GPS ad paucos metros exactum est, ita ut una feriarum photographia domum prodere possit. Numerus serialis est ille fortis: idem est in omni photographia quam ea photomachina umquam cepit, itaque imaginem anonymam ad copiam quam sub tuo nomine posuisti ligat. Imago minuta semel generatur et saepe non renovatur, ita ut photographia recisa ut aliquem tollat originale non recisum intra se adhuc ferre possit.",
    },
    FileType {
        name: "Photographia telephoni (HEIC, HEIF, AVIF)",
        carries: "Eadem EXIF et XMP ac JPEG, et telephona moderna diligentia sunt: GPS exacta, exemplar instrumenti, capiendi optiones, et interdum mappa profunditatis aut series imaginum.",
        identifies: "Omnia quae casus JPEG describit, et quia recta e telephono cum loco accenso venit, GPS plerumque adest et exacta est. Exemplar instrumenti una cum systematis operandi particulis coartat cuius telephonum fuerit.",
    },
    FileType {
        name: "Imago PNG (.png)",
        carries: "Fragmenta textus quae commentarios liberos, programma quod documentum scripsit, tempus creationis, et interdum plenum fragmentum EXIF ex originali photographia translatum tenent.",
        identifies: "Homines PNG «purum» esse putant quia saepe pro imaginibus ecranicis adhibetur, sed conversores saepe EXIF originalis photographiae, cum GPS, in PNG ferunt. Campus programmatis instrumentum et versionem quam adhibuisti prodit.",
    },
    FileType {
        name: "WebP et GIF",
        carries: "WebP EXIF et XMP ut JPEG fert. GIF fragmenta commentariorum et applicationum fert, quae pro nominibus auctorum, textibus programmatum et XMP adhibita sunt.",
        identifies: "Eadem loci et instrumenti fabula pro WebP. Pro GIF plerumque textus auctoris aut programmatis est potius quam GPS, sed id adhuc instrumentum, rationem, aut hominem nominat.",
    },
    FileType {
        name: "TIFF (.tif)",
        carries: "TIFF est continens super quo ipsum EXIF aedificatur, itaque totam copiam tenet: GPS, photomachinae notam, exemplar et numerum serialem, temporum notas, et imaginem minutam insertam.",
        identifies: "Idem ac JPEG, et TIFF communis est pro scansionibus et opere professionali, ubi numerus serialis scanneri aut photomachinae et nota temporis exacta documentum ad unam machinam referre possunt.",
    },
    FileType {
        name: "Photomachinae raw (CR2, CR3, NEF, ARW, RAF, DNG, …)",
        carries: "Omnia quae JPEG fert, et plura. Maior nota fabricatoris numerum serialem internum et numerum obturamenti tenet. Plena JPEG praevisio intus inserta est, cum suo EXIF et GPS.",
        identifies: "Raw peior est effluvium quam JPEG eiusdem ictus, non melior. Numerus obturamenti photographias tuas re vera in ordine quo eas cepisti numerat. Nota fabricatoris data quae conversori ad documentum explicandum necessaria sunt tenet, ideo hoc instrumentum eam servat et numerus serialis in ea plerumque manet — quare raw numquam in loco plene purgari potest. Ut numerum serialem removeas, raw in JPEG explica et id purga.",
    },
    FileType {
        name: "Imago vectorialis SVG (.svg)",
        carries: "SVG est XML, itaque rationes retractatoris fert: programma pingendi et versionem, stratorum et fenestrarum dispositionem, interdum viam documenti, fragmentum metadatorum cum auctore et licentia, et relationes ad documenta externa.",
        identifies: "Campi retractatoris et quaevis via documenti inserta nomen usoris aut structuram scriniorum quae te nominat ferre possunt. Relatio externa imaginem facit rem a moderatro petere cum aliquis eam aperit, quod illi moderatro renuntiat eam visam esse. Scripta in SVG currere possunt cum in navigatro aperitur.",
    },
    FileType {
        name: "Documentum comes XMP (.xmp)",
        carries: "Documentum quod nihil nisi metadata est, iuxta photographiam a programmate retractandi scriptum: auctor, ius auctoris, GPS, dies quibus photographia capta et retractata est, plena historia retractationis, identificatores catalogi, et numerus serialis photomachinae.",
        identifies: "Homines comitem exsistere obliviscuntur et eum una cum photographia communicant. Fert identitatem a qua photographia purgata est. Historia retractationis documentum ad certam sessionem in certa machina ligat, et identificatores catalogi id ad unam photographiarum bibliothecam ligant.",
    },
    FileType {
        name: "Documentum PDF (.pdf)",
        carries: "Fragmentum informationis documenti cum auctore, programmate quod id produxit, et temporibus creationis et mutationis. Saepe etiam fragmentum XMP, et historiam retractationum gradatim additarum.",
        identifies: "Campus auctoris plerumque automatice ex ratione sub qua programma installatum est impletur, itaque saepe nomen verum aut nomen usoris corporati est quod scriptor numquam typis expressit neque vidit. Historia retractationum priores documenti versiones tenere potest — quo modo PDF «expurgata» textum sub quadris nigris effluere fecerunt.",
    },
    FileType {
        name: "Word, Excel, PowerPoint, OpenDocument",
        carries: "Nomina auctoris et ultimi retractatoris, societas, totum retractandi tempus, identificatores retractationum servatarum, mutationes vestigatae cum nominibus omnium qui retractaverunt, viae exemplarium, et quaevis imagines insertae, quae suum EXIF servant.",
        identifies: "Campi auctoris et ultimi retractatoris homines veros aut nomina rationum nominant. Identificatores retractationum servatarum sunt numeri fortuiti qui inter documenta in eadem sessione in eadem machina retractata congruunt, quod documenta nihil aliud commune habentia ligat. Mutationes vestigatae prodere possunt quis quid scripserit, et photographia inserta suum GPS secum affert.",
    },
];

const RAW_LA: &[Section] = &[
    Section {
        heading: "Quid documentum raw re vera sit",
        body: "Documentum raw est lectura fere non tractata sensoris imaginum photomachinae, antequam photomachina eam in imaginem convertit. Non est imago spectabilis sensu ordinario. Nullos colores fixos habet, nullam contrarietatis curvam et nullam acutiem applicatam; propius est negativo photographico quam impressioni perfectae, et a programmate «explicandum» est antequam alicui simile videtur.\n\nQuia plena sensoris data fere nihil abiecto tenet, raw magnum est et solum a programmate quod illius photomachinae formam intellegit aperiri potest. Id est tota ratio raw capiendi: nihil adhuc decretum aut abiectum est, ita ut photographus illas electiones postea facere possit.",
    },
    Section {
        heading: "JPEG non retractatum non est raw",
        body: "Hoc est error frequentissimus et carissimus, ideo apertum esse operae pretium est. JPEG recta e telephono aut photomachina, quod numquam in retractatore aperuisti, non est documentum raw. Iam intra photomachinam explicatum est: sensoris data in colores conversa sunt, contrarietatis curva et acuties applicatae, et exitus compressus et ad partem magnitudinis originalis abiectus. «A te non retractatum» non idem est ac «raw».\n\nModus certus ea distinguendi est genus documenti, non quomodo imago videtur. Raw extensiones fabricatoris proprias habent: Canon CR2 et CR3, Nikon NEF et NRW, Sony ARW, Panasonic RW2, Olympus ORF, Fujifilm RAF, Pentax PEF, universale Adobe DNG, et duodecim alia. Si documentum in .jpg aut .jpeg desinit, JPEG est, quidquid aliud de eo verum est. Si dubitas, instrumentum tibi dicit quam formam ex ipso documenti contento detexerit, nomine neglecto.",
    },
    Section {
        heading: "Cur raw plura removenda ferat, non pauciora",
        body: "Raw peius periculum privatati est quam JPEG eiusdem photographiae, non melius. Plenum fragmentum EXIF fert, et eius nota fabricatoris, ubi numerus serialis sensoris et numerus obturamenti habitant, plerumque maior et particularior est quam in documento tractato.\n\nRaw etiam fere semper plenam JPEG praevisionem photographiae explicatae continet, intus insertam ut programma ictum monstrare possit sine sensoris datis decodendis. Illa praevisio suum EXIF, suum GPS habet, et versionem sceni ante retractationem etiam monstrare potest. Est documentum intra documentum, et id quoque purgandum est. Hoc instrumentum metadata intra illas insertas praevisiones necnon in ipsis raw notis invenit et vacuat.",
    },
    Section {
        heading: "Duo gradus purgandi, et cur raw sit inferior",
        body: "Imagines et documenta ordinaria ab initio reficiuntur: instrumentum indicem partium servandarum tenet, solas eas in novum documentum copiat, et ex constructione nihil aliud superest. JPEG, PNG, WebP, HEIC, AVIF, GIF, TIFF, PDF et documenta Office hoc modo purgantur, ad exitum «Completum».\n\nRaw ita tractari non potest. Eius sensoris imago in sub-sectionibus fabricatoris propriis habitat quarum dispositio non documentata est et pro quoque fabricatore diversa, et probatio in veris documentis multarum notarum ostendit refectionem documentum reddere quod amplius non aperitur. Raw non est quod iterum capere possis, ideo instrumentum illud periculum non suscipiet. Raw in loco potius purgantur: documentum retractatur, non reficitur, nihil movetur, et longitudo non mutatur. Hoc semper est exitus «Pro viribus».",
    },
    Section {
        heading: "Quid exacte in raw mutetur, et quid non",
        body: "Quia margo pro raw angustus est, instrumentum de omnibus tribus generibus accuratum est, et idem tibi de omni documento quod purgat dicit.\n\nREMOTA, zeris superscripta: locus GPS; dies et hora quibus photographia capta et ultimo mutata est; nomina possessoris et artificis; quodvis fragmentum XMP aut IPTC; campi standardae numeri serialis et imaginis-ID; et metadata intra insertam praevisionem. Renuntiatio exacte enumerat quae ex his in documento tuo inventa sint.\n\nSERVATA de industria: photomachinae nota et exemplar, et nota fabricatoris (infra explicata).\n\nNON TACTA: ipsa sensoris imaginis data, et documenti facultas explicandi. Imago bit pro bit eadem est; sola informatio circa eam mutata est.",
    },
    Section {
        heading: "Cur nota fabricatoris servetur, et quid id effluat",
        body: "Nota fabricatoris est fabricatoris fragmentum privatum. Numerum serialem internum photomachinae et numerum obturamenti, qui identificant, re vera tenet. Bonum esset id removere.\n\nSed fabricatores etiam optiones quas conversor raw ad documentum explicandum eget in eodem fragmento servant: nigri et albi gradus, sensoris colorum coli dispositionem, albi aequilibrium, lentis correctiones. In veris documentis Canon, Nikon, Olympus, Pentax et aliorum, notam fabricatoris removere aut raw aperiri desinere fecit aut quomodo decoderetur mutavit. Documentum corrumpere est unicus exitus quem hoc instrumentum recusat, ideo nota fabricatoris manet.\n\nConsecutio vera: in raw numerus serialis internus in nota fabricatoris plerumque superest. Quantum informationis identificantis maneat ideo a tua photomachina pendet, quia aliquae notae numerum serialem etiam in campum standardum scribunt, qui removetur, dum aliae eum solum in nota fabricatoris tenent, quae non removetur. Si ille numerus serialis tibi refert, via tuta infra utere.",
    },
    Section {
        heading: "Via tutissima, et quae photomachinae comprehendantur",
        body: "Si quod eges est imaginem communicare potius quam negativum raw, raw primum in JPEG aut PNG explica et id purga. JPEG ad exitum «Completum» reficitur et nullam notam fabricatoris, insertam praevisionem, aut sub-sectiones venditoris quas raw habet fert. Ea est via numerum serialem quem raw servat removendi.\n\nInstrumentum raw agnoscit a Canon (CR2, CR3), Nikon (NEF, NRW), Sony (ARW, SR2), Fujifilm (RAF), Olympus et OM System (ORF), Panasonic (RW2), Pentax (PEF), Leica, Samsung (SRW), Adobe (DNG), Epson (ERF), GoPro (GPR), et medii formati backs a Hasselblad, Phase One et Leaf, inter alia. Quodque ex contento suo identificatur, numquam ex nomine documenti. Sigma X3F dispositionem utitur quam instrumentum nondum resolvit, ideo intactum relinquitur et ut non purgatum renuntiatur, potius quam male purgatum.",
    },
];

const FIRST_USE_LA: &str = "Photomachina tua leve exemplar in pixellis omnis photographiae quam capit relinquit. Ex parvis fabricationis differentiis inter sensores lucis oritur, per totam photomachinae vitam fixum est, et non est metadatum. EXIF removere nihil ad id facit.\n\nAdhiberi potest ut ostendatur duas photographias ex eadem photomachina venisse. Id refert si sub tuo nomine edis et etiam aliquid anonyme edere vis.\n\nQuid hoc facit: imaginem a strepitu purgat, minorem facit, paulum strepitus addit, et iterum comprimit. Simul haec minuunt quam firmiter exemplar congruere possit.\n\nQuid non facit: exemplar removere. Nemo id polliceri potest. Hoc fiduciam congruentiae minuit; eam impossibilem non facit.\n\nQualitatem etiam imaginis constat. Photographiae tuae molliores et minores erunt. Ideo exstinctum manet nisi id accendas.";

const PRNU_LA: &[Section] = &[
    Section {
        heading: "Quid vestigium sensoris sit",
        body: "Sensor photomachinae est craticula ex milionibus puteolorum lucis sensibilium, in silicio incisorum. Fabricatio eos perfecte pares facere non potest, ideo quisque ad lucem paulum aliter quam vicini respondet. Aliqui paulo clariores, aliqui paulo obscuriores leguntur, parte centesima.\n\nIlla varietas fixa est. Decernitur cum sensor fabricatur et per totam photomachinae vitam non mutatur. Omnis photographia quam photomachina capit eam fert, leviter in claritatem cuiusque pixelli multiplicatam. Vocatur Photo Response Non-Uniformity, sive PRNU.\n\nConsecutio practica: est numerus serialis in ipsam imaginem scriptus potius quam in documenti campos informationis. EXIF removere eum non tangit. Neque documentum renominare, id imagine ecranica capere, aut per applicationem quae metadata tollit mittere.",
    },
    Section {
        heading: "Quomodo contra aliquem adhibeatur",
        body: "Analyticus versionem photographiae sine strepitu aestimat, eam subtrahit ut reliquum tenuis particulae relinquat, et illud reliquum cum exemplari referentiae photomachinae comparat. Fortis correlatio dicit photographiam ex illo sensore venisse.\n\nQuod refert est eos exemplar referentiae primum egere. Aedificatur aut ex ipsa photomachina aut ex photographiarum copia iam nota ex ea venire. Itaque hoc est impetus ligandi potius quam identificandi. Nemo photographiam anonymam spectat et nomen ex pixellis derivat.\n\nScaenarium verisimile hoc est: aliquis opus sub suo nomine edit, deinde aliquid anonyme edit, et utrumque eadem photomachina captum est. Analytico non opus est photographiam anonymam identificare. Solum ei opus est ostendere eam ex eodem sensore venisse ac publicae.",
    },
    Section {
        heading: "Cur strepitum tollere adiuvet",
        body: "Exemplar in tenui, alti frequentiae particula habitat, quod est exacte id quod strepitus-tollens petit.\n\nInstrumenta quae PRNU detegunt operantur imaginem a strepitu purgando et reliquum quod subtraxerunt tenendo. Itaque a strepitu purgare et imaginem potius tenere est operatio prorsus contraria, illi imaginis parti applicata ubi vestigium fortissimum est.\n\nPrimum fit, in plena resolutione, quia ibi exemplar maxime aestimabile est et ideo maxime oppugnandum.",
    },
    Section {
        heading: "Cur imaginem minuere maxime adiuvet",
        body: "Correlatio pendet a singulis pixellis photographiae cum congruenti puncto exemplaris referentiae componendis. Minuere illam congruentiam frangit.\n\nCum imago magnitudine mutatur, quodque pixellum exitus ex pluribus pixellis ingressus miscetur. Exemplar fixum cum vicinis mediatur et per novam craticulam quae sensoris amplius non congruit oblinitur. Ex quattuor operationibus hic, haec correlationem maxime minuit.\n\nPost strepitum tollere fit, ut retexere sit ultimum quod craticulam pixellorum tangit.",
    },
    Section {
        heading: "Cur paulum strepitus addatur",
        body: "Post strepitum tollere et minuere, aliquod vestigium exemplaris manet. Paulum novi strepitus fortuiti addere rationem signi ad strepitum cuiusvis aestimationis quam analyticus ex imagine facere potest minuit.\n\nId quod adest non delet. Id quod adest difficilius metiendum facit, quod pro probatione statistica in margine fere idem valet.",
    },
    Section {
        heading: "Cur iterum comprimere sit gradus infirmissimus",
        body: "Consilium commune est photographiam comprimere et decomprimere ut vestigium deleatur. Id est operationum hic minime efficax.\n\nPRNU mediocrem JPEG compressionem commode superat. Compressio cum damno aliquam alti frequentiae particulam abicit, quod paulum adiuvat, sed per se numquam satis futurum erat. Ut gradus ultimus includitur potius quam ei fiditur.",
    },
    Section {
        heading: "Quid non operatur: color",
        body: "Albi aequilibrium mutare, colorem inducere, aut singulorum canalium augmentum mutare nihil omnino facit.\n\nDetectores correlatione normalizata utuntur, quae quamvis aequabilem scalationem aut inclinationem ante comparationem dividit. Aequabilis coloris mutatio est exacte illa transformatio, ideo a mathematica ante comparationem removetur. Colorum accurationem constat et nullam protectionem emit.\n\nHoc scribitur quia est opinio intuitiva quae falsa esse contingit, et in multis consiliis online apparet.",
    },
    Section {
        heading: "Limes verus",
        body: "Hoc correlationem minuit. Vestigium non removet, et nihil in hac applicatione tibi umquam dicet se id removisse.\n\nAnalyticus forensicus cum forti exemplari referentiae, multis imaginibus exemplaribus, et tempore potest scalationis factores compensare et per resectiones quaerere. Contra id, hae operationes pretium augent et fiduciam congruentiae minuunt. Congruentiam impossibilem non faciunt.\n\nEst etiam pretium a tua parte. Omnis optio hic photographiam deterit: mollior particula, pauciora pixella, plus compressionis. Illud commercium tuum est facere, quare hoc exstinctum manet nisi id accendas.\n\nSi salus tua a non-ligabilitate pendet, mensura fortior est omnino non ex eadem photomachina sub duabus identitatibus a principio edere.",
    },
];

const MYTHS_LA: &[Myth] = &[
    Myth {
        claim: "Photographiam per applicationem nuntiorum mittere omnia removet.",
        reality: "Pleraque magna suggesta EXIF tollunt cum imaginem ut photographiam mittis, ideo locus plerumque abit. Duae cautiones. Idem documentum ut documentum aut adiunctum mittere originale intactum, cum metadatis omnibus, imponit, et homines illam optionem pro meliore qualitate eligunt sine intellectu quid aliud comitetur. WhatsApp et Telegram ambo ita se gerunt; Signal metadata etiam in modo documenti tollit, quod est vera differentia inter ea. Et nullum eorum exemplar sensoris in pixellis tangit, quia id non est metadatum.",
    },
    Myth {
        claim: "Imaginem ecranicam capere metadata removet.",
        reality: "Fere verum, et exemplar sensoris etiam turbat, quia id quod ecranus monstravit capis potius quam id quod sensor recordatus est. Sed imago ecranica sua nova metadata fert, qualitas multo peior est quam recti exemplaris purgati, et error communis est imaginem ecranicam pro salute capere et deinde originale casu mittere.",
    },
    Myth {
        claim: "Documentum renominare, aut in zip ponere, metadata removet.",
        reality: "Neutrum quicquam omnino facit. Nomen documenti non est pars contenti documenti, et archivum documentum exacte servat ut ex altera parte immutatum exeat. Id est tota ratio archivi.",
    },
    Myth {
        claim: "Photographia mea est documentum raw quia eam non retractavi.",
        reality: "Photographia quam non tetigisti adhuc non est documentum raw. JPEG recta e telephono aut photomachina iam intra photomachinam explicatum est: colores, contrarietas et acuties applicatae, deinde compressum. Raw est sensoris data ante illa omnia, in forma fabricatoris propria (CR2, CR3, NEF, ARW, DNG, RW2, ORF, RAF et aliae) quae speciali programmate ad aperiendum eget. Genus documenti id decernit, non utrum imaginem retractaveris. Hic refert quia raw plura removenda fert, et hoc instrumentum raw solum pro viribus purgare potest, non reficere. Vide «Documenta raw photomachinae».",
    },
    Myth {
        claim: "In PNG convertere metadata tollit.",
        reality: "PNG sua metadatorum fragmenta habet, pleno EXIF fragmento et campis textus liberi inclusis, et multi conversores notas transferunt potius quam eas abiciunt. Formam mutare non idem est ac informationem removere.",
    },
    Myth {
        claim: "Servitia loci exstinxi, ideo photographiae meae bene se habent.",
        reality: "Id GPS-coordinatas removet, quae sunt maximum singulum elementum, et faciendum est. Cetera omnia manent: photomachinae exemplar, lens, numerus serialis in nota fabricatoris, nota temporis exacta, imago minuta inserta, et historia retractationis.",
    },
    Myth {
        claim: "Photographiam tantum comprime et vestigium photomachinae abiit.",
        reality: "Hoc est consilium frequentissimum et infirmissimum operationum utilium. Exemplar mediocrem JPEG compressionem commode superat. Compressio in margine adiuvat; magnitudo mutanda et strepitus tollendus verum opus faciunt.",
    },
    Myth {
        claim: "Colores aut albi aequilibrium mutare vestigium vincit.",
        reality: "Nihil facit. Comparatio est correlatio normalizata, quae quamvis aequabilem per-canalem scalationem aut inclinationem ante comparationem dividit. Aequabilis coloris mutatio ab arithmetica removetur antequam ullum effectum habere potest.",
    },
    Myth {
        claim: "Photographiam resecare vestigium vincit.",
        reality: "Adiuvat, quia compositionem a qua comparatio pendet movet, sed analyticus per possibiles resectionis positiones quaerere potest. Exemplar etiam intactum in quibuscumque pixellis manentibus relinquit.",
    },
    Myth {
        claim: "Vestigium sensoris theoreticum est, aut aliquid ex pelliculis.",
        reality: "Est ars documentata cum litteris investigationis ad annum 2006 redeuntibus et usu in veris causis. Eam ut fictionem tractare tantus error est quantus eam ut infallibilem tractare.",
    },
    Myth {
        claim: "Vestigium sensoris significat te semper identificari posse.",
        reality: "Aeque falsum in alteram partem. Congruentia exemplar referentiae tuae certae photomachinae eget, ex ipso instrumento aut ex photographiis iam notis tuas esse aedificatum. Sine eo nihil est cum quo comparetur. Photographias inter se ligat; nomen non producit.",
    },
    Myth {
        claim: "Documentum purum est, ideo ad me retrahi non potest.",
        reality: "Documentum purgare id quod intra documentum est tractat. Si id imposuisti dum inscriptus eras, suggestum suum recordum habet quae ratio quid et quando miserit, et illud recordum per processum iuridicum accessibile est quantumvis purum documentum fuerit. Aliqua suggesta suum identificatorem etiam in imagines in transitu scribunt. Vide «Quid hoc instrumentum attingere non possit».",
    },
    Myth {
        claim: "Minuere et iterum comprimere suggestum ab imagine agnoscenda prohibet.",
        reality: "Non. Suggesta imagines cum perceptualibus hashis congruunt, quae specialiter aedificata sunt ut magnitudinis mutationem, iteratam compressionem et parvas retractationes superent. Vestigii minutio in hac applicatione exemplar sensoris turbat; suggestum a videndo duo documenta eandem photographiam esse non prohibet. Alius impetus, alia defensio.",
    },
    Myth {
        claim: "Hoc solum refert si diurnarius aut actor es.",
        reality: "Damnum verum frequentissimum est domesticum. Photographia publice posita coordinatas loci ubi capta est ferre potest, et imago minuta versionem imaginis quae de causa recisa est ferre potest.",
    },
];

const EVIDENCE_LA: &[Section] = &[
    Section {
        heading: "Unde ars veniat",
        body: "Vestigium sensoris a Lukáš, Fridrich et Goljan in «Digital Camera Identification from Sensor Pattern Noise», anno 2006 in IEEE Transactions on Information Forensics and Security edito, constitutum est. Est charta fundamentalis et basis huius campi manet.\n\nMethodus eorum: exemplar referentiae photomachinae aedificare multas photographias ex ea capiendo, unamquamque a strepitu purgando, reliquum tenendo, et illa reliqua mediando ita ut pars fixa se confirmet dum pars fortuita se aboleat. Deinde photographia quaesita eodem modo a strepitu purgatur et eius reliquum cum illa referentia comparatur.",
    },
    Section {
        heading: "Cur exemplar referentiae sit tota fabula",
        body: "Quia referentia mediando per multas imagines ex una photomachina aedificatur, analyticus iam aut instrumentum aut corpus photographiarum ei attributarum habere debet.\n\nHoc est unicum maxime momenti factum de minacia, et id quod saepissime omittitur. Ars respondet «venerintne haec ex eodem sensore?» Non respondet «cuius est haec photomachina?» nisi aliquis iam responsum praebuit.",
    },
    Section {
        heading: "Quid testimonium de magnitudine mutanda dicat",
        body: "Hic honestas maxime refert, quia magnitudinem mutare est praecipuum quod hoc instrumentum facit.\n\nLitterae consentiunt: identificatio ex imaginibus minutis possibilis manet, sed effectus notabiliter deterioratur. Magnitudinem mutare tamquam filtrum humilis frequentiae agit, et diversi scalationis factores diversas signi partes servant. Analyticus qui scalationis factorem scit aut coniectat eum compensare potest.\n\nItaque minuere est operatio hic efficacissima, et adhuc non est clades. «Notabiliter deteriorat» est honesta descriptio, et ea est quam haec applicatio adhibet.",
    },
    Section {
        heading: "Quid testimonium de contra-forensica dicat",
        body: "Methodi contra-forensicae contra vestigia sensoris sunt campus investigationis activus potius quam problema solutum. Rationes editae includunt augere una interpolationis methodo et minuere alia, ita ut pixellorum valores verisimiles sint sed cum originali craticula amplius non compositi, et plures formas strepitus supprimendi et iniciendi.\n\nNulla in illis litteris ut garantia praesentatur. Describuntur ut attributionem difficiliorem facientes, quae est alia assertio, et ea est assertio hic facta.",
    },
    Section {
        heading: "Ubi investigatores dissentiant",
        body: "Fides sub veris condicionibus disputatur. Recens opus quaerit quam bene ars in modernis telephonis intellegentibus se teneat, ubi gravis computatoria tractatio, acris strepitus reductio et digitalis stabilizatio omnia exemplar turbant antequam documentum omnino scribitur.\n\nEst etiam disputatio permanens utrum campus normam stabilitam pro causis habeat. Quisquis tibi dicit responsum simplex esse, in utramvis partem, testimonium praecurrit.",
    },
    Section {
        heading: "Quid hoc tibi significet",
        body: "Metadata removere est pars quae probari potest. Informatio in definitis locis est, removetur, et exitus alio instrumento probari potest.\n\nVestigii sensoris reductio est statistica. Fiduciam congruentiae minuit quantitate quam nemo pro tua certa photographia, photomachina et adversario exacte enuntiare potest.\n\nEa sunt diversa genera assertionis et haec applicatio ea ea de causa visibiliter separata tenet. Si salus tua a non-ligabilitate pendet, opus vestigii ut unum stratum inter plura tracta, non ut id quod problema solvit.",
    },
];

const BEYOND_THE_FILE_LA: &[Section] = &[
    Section {
        heading: "Documentum purum non est impositio anonyma",
        body: "Cetera omnia in hac tabella de informatione intra documentum lata sunt. Haec sectio de informatione alibi tenta, quae idem documentum describit.\n\nSi photographiam imponis dum inscriptus es, suggestum scit quae ratio eam imposuerit, quando, et ex qua inscriptione, quantumvis purum documentum fuerit. Metadata removere impositionis recordum non removet. Numquam a principio in documento fuit.\n\nHoc instrumentum ad quicquam eorum attingere non potest, neque ullum instrumentum huius generis potest.",
    },
    Section {
        heading: "Suggesta suos identificatores addunt",
        body: "Aliqua servitia non solum metadata tollunt, sed sua inscribunt.\n\nFacebook identificatorem in fragmento IPTC impositarum imaginum a circiter anno 2014 inseruit, in campo transmissionis relationibus destinato, cum valoribus a «FBMD» incipientibus. A investigatore securitatis anno 2019 inventus est, et IPTC, qui normam adhibitam possident, documentationem huius moris quaesiverunt nec ullam invenerunt.\n\nEffectus practicus: imago a suggesto descripta notam quam suggestum interpretari potest ferre potest, in documento quod aliter nudatum videtur. Haec applicatio fragmenta IPTC omnino removet, ideo imaginem descriptam per eam ducere illam quoque notam removet. Quod facere non potest est exemplar quod suggestum servavit removere.",
    },
    Section {
        heading: "Quid processus iuridicus obtinere possit",
        body: "Exactum mechanismum scire operae pretium est, quia saepe nimis laxe describitur.\n\nSecundum edita Meta praecepta, subpoena in investigatione criminali cogit basica subscriptoris recorda: nomen, servitii longitudinem, inscriptiones electronicas, et recentes ingressus inscriptiones. Cogere contenta servata rationis, quae nuntios, photographias et videos includunt, mandatum perquisitionis in probabili causa monstranda eget. Contentum est altior gradus quam subscriptoris particulae, non idem.\n\nRetentio elementum temporis etiam habet. Meta recorda pendente processu iuridico servat, sed petitio conservationis antequam materia deletur advenire debet. Data iam abita abierunt.\n\nItaque responsum ad «potestne aliquis hoc documentum ad eum qui misit retrahere» est: recto processu iuridico, in recta iurisdictione, intra retentionis fenestram, frequenter ita. Id nihil ad documenti metadata pertinet.",
    },
    Section {
        heading: "Hashae perceptuales, et limes huius instrumenti",
        body: "Magna suggesta imagines cum perceptualibus hashis congruunt, quae destinatae sunt ut exacte illas mutationes superent quae ordinariam summam probationis frangunt: magnitudinem mutare, iterum comprimere, parvas coloris inclinationes, parvas resectiones.\n\nHoc directam consecutionem pro vestigii reductione hic oblata habet. Strepitum tollere, minuere et iterum codicare destinata sunt ut exemplar sensoris turbent, et perceptualis hashatio aedificata est ut ad illas ipsas operationes indifferens sit. Exemplar lotum adhuc suo originali sub illo comparationis genere congruet.\n\nHi sunt diversi impetus cum diversis defensionibus. Nihil in hac applicatione est defensio contra suggestum agnoscens duas imagines eandem picturam esse.",
    },
    Section {
        heading: "Omne aliud exemplar",
        body: "Documentum quod purgas est unum exemplar. Originale adhuc in tuo imaginum volumine est, verisimiliter in nubis subsidiario, fortasse in ipsa applicationis nuntiorum memoria, et postquam id mittis, in alterius instrumento ubi nullam omnino potestatem habes.\n\nExemplar ante mittendum purgare operae pretium est. Non idem est ac informationem exsistere desinere.",
    },
    Section {
        heading: "Quid re vera adiuvet",
        body: "Si sollicitudo est documentum, documentum purga. Ad id hoc instrumentum destinatum est et id bene facit.\n\nSi sollicitudo est impositionem ad te non retrahibilem esse debere, documentum minimum est. Ratio, coniunctio per quam facta est, solvendi methodus post rationem, et instrumentum ex quo venit omnia plus referunt, et nullum eorum hic tractatur.\n\nDe illo limite apertum esse utilius est quam instrumentum quod omnia se texisse innuit.",
    },
];
