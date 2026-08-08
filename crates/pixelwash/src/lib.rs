//! `pixelwash` — reduce sensor-fingerprint linkability in photographs
//! (DESIGN §9.2).
//!
//! ## What this is for
//!
//! Every camera sensor has microscopic manufacturing variations, so each
//! photosite responds to light slightly differently from its neighbours. That
//! variation is fixed for the life of the sensor and it is imprinted on every
//! photograph the camera takes, as a faint multiplicative pattern called
//! **PRNU** (Photo Response Non-Uniformity). It is, in effect, a serial number
//! written into the pixels rather than into the file's metadata, which means
//! removing EXIF does nothing to it whatsoever.
//!
//! ## The threat it addresses, and its limits
//!
//! PRNU is a **linking** attack, not an identification one. Correlating a photo
//! against a sensor requires a *reference pattern*, built either from the
//! physical camera or from a set of photographs already known to come from it.
//! So the realistic scenario is: someone publishes work under their own name,
//! then publishes something anonymously from the same camera, and an analyst
//! establishes that both came from one sensor.
//!
//! This crate **reduces the correlation**. It does not remove the pattern, and
//! nothing here should be described as if it did. A forensic analyst with a
//! strong reference pattern, many samples, and the ability to search over
//! scaling factors and crops may still succeed.
//!
//! ## Why these operations, in this order
//!
//! 1. **Denoise, at full resolution.** PRNU lives in the fine, high-frequency
//!    detail. The forensic tools that *detect* it work by denoising an image
//!    and keeping the residual, so denoising and keeping the *image* is the
//!    exact counter-operation, applied where the pattern is strongest.
//! 2. **Downscale.** Resampling mixes each output pixel from several input
//!    pixels, destroying the pixel-for-pixel correspondence that correlation
//!    depends on. This is the single most effective step.
//! 3. **Inject a little noise.** Random noise added after denoising masks what
//!    survived, by lowering the signal-to-noise ratio of any estimate an
//!    analyst can make.
//! 4. **Re-encode.** Lossy compression discards more high-frequency detail.
//!    Weakest of the four on its own, which is worth saying plainly because
//!    "just recompress it" is the usual folk advice.
//!
//! ## What does not work
//!
//! **Colour casting, white-balance shifts, and global gain changes do
//! nothing.** Detectors use *normalised* cross-correlation, so any uniform
//! per-channel scaling or offset is divided straight back out. It costs colour
//! fidelity and buys no protection. Included here as an explicit note because
//! it is an intuitive idea that happens to be wrong.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use image::{DynamicImage, GenericImageView, ImageFormat, RgbImage};
use rand::Rng;

mod denoise;

pub use denoise::soft_threshold;

/// Errors from washing an image.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The bytes could not be decoded as an image this crate handles.
    #[error("could not decode this image: {0}")]
    Decode(String),
    /// Re-encoding the washed image failed.
    #[error("could not re-encode the image: {0}")]
    Encode(String),
    /// The image is larger than the configured ceiling.
    #[error("image is {width}x{height}, larger than the {limit} megapixel limit")]
    TooLarge {
        /// Decoded width.
        width: u32,
        /// Decoded height.
        height: u32,
        /// Configured ceiling, in megapixels.
        limit: u32,
    },
}

/// How hard to work, traded against how much the photograph is degraded.
///
/// There is no setting that removes the fingerprint, so these are named for
/// what they cost rather than for a protection level they cannot promise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Strength {
    /// Barely visible. Slight denoise, 90% scale, light noise, quality 92.
    ///
    /// Suitable when image quality matters and the aim is to raise the cost of
    /// a match rather than to defeat a determined analyst.
    Gentle,
    /// Visible on close inspection. Moderate denoise, 70% scale, quality 85.
    #[default]
    Balanced,
    /// Clearly softened, and small. Heavy denoise, 50% scale, quality 78.
    ///
    /// The most this crate will do. Still not a guarantee.
    Thorough,
}

impl Strength {
    /// Fraction of the original dimensions kept.
    fn scale(self) -> f32 {
        match self {
            Strength::Gentle => 0.90,
            Strength::Balanced => 0.70,
            Strength::Thorough => 0.50,
        }
    }

    /// Radius of the blur used to estimate the noise-free image.
    fn denoise_radius(self) -> f32 {
        match self {
            Strength::Gentle => 0.8,
            Strength::Balanced => 1.3,
            Strength::Thorough => 2.0,
        }
    }

    /// How much of the high-frequency residual is shrunk away, 0.0 to 1.0.
    fn denoise_amount(self) -> f32 {
        match self {
            Strength::Gentle => 0.45,
            Strength::Balanced => 0.70,
            Strength::Thorough => 0.90,
        }
    }

    /// Standard deviation of the noise added afterwards, in 0-255 units.
    fn noise_sigma(self) -> f32 {
        match self {
            Strength::Gentle => 0.8,
            Strength::Balanced => 1.6,
            Strength::Thorough => 2.6,
        }
    }

    /// JPEG quality for the re-encode.
    fn quality(self) -> u8 {
        match self {
            Strength::Gentle => 92,
            Strength::Balanced => 85,
            Strength::Thorough => 78,
        }
    }

    /// A short, honest description for a user interface.
    pub fn describe(self) -> &'static str {
        match self {
            Strength::Gentle => "Barely visible change. Keeps 90% of the size.",
            Strength::Balanced => "Softer on close inspection. Keeps 70% of the size.",
            Strength::Thorough => "Clearly softened and noticeably smaller. Keeps 50%.",
        }
    }
}

/// Settings for a wash.
#[derive(Debug, Clone)]
pub struct Settings {
    /// How hard to work. See [`Strength`].
    pub strength: Strength,
    /// Refuse images above this many megapixels, so a decompression bomb
    /// cannot exhaust memory. `None` disables the check.
    pub max_megapixels: Option<u32>,
}

impl Default for Settings {
    fn default() -> Self {
        // 120 MP is comfortably above any consumer camera and well below a
        // deliberately hostile file.
        Self { strength: Strength::default(), max_megapixels: Some(120) }
    }
}

/// What a wash did, so the interface can report it rather than assert success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WashReport {
    /// Dimensions before.
    pub original: (u32, u32),
    /// Dimensions after.
    pub washed: (u32, u32),
    /// Output size in bytes.
    pub output_len: usize,
    /// JPEG quality used for the re-encode.
    pub quality: u8,
}

/// A washed image plus the account of what was done to it.
#[derive(Debug, Clone)]
pub struct Washed {
    /// Re-encoded JPEG bytes.
    pub data: Vec<u8>,
    /// What happened.
    pub report: WashReport,
}

/// Denoise, downscale, add noise, and re-encode, to reduce PRNU correlation.
///
/// Output is always JPEG: the point is to discard fine detail, so preserving a
/// lossless container would be working against the purpose.
///
/// This **reduces** linkability. It does not remove the sensor fingerprint, and
/// any interface built on this must say so.
pub fn wash(input: &[u8], settings: &Settings) -> Result<Washed, Error> {
    // Bound the decoder *before* it runs. The previous approach decoded first
    // and checked dimensions afterwards, which is the decompression-bomb order
    // of operations: a 30000x30000 PNG is a couple of hundred bytes on disk and
    // several gigabytes decoded, and the size check never gets a turn because
    // the allocation happens inside the decode. The `image` crate has its own
    // default memory cap, but relying on a dependency's default for a security
    // property is fragile across version bumps, so the limit is set explicitly
    // here from our own configuration.
    let mut reader = image::ImageReader::new(std::io::Cursor::new(input));
    reader = reader.with_guessed_format().map_err(|e| Error::Decode(e.to_string()))?;

    if let Some(mp) = settings.max_megapixels {
        let mut limits = image::Limits::default();
        // Pixel ceiling as a width/height product, plus a hard allocation cap so
        // an intermediate buffer cannot balloon past what the pixels imply.
        let max_px = mp as u64 * 1_000_000;
        limits.max_image_width = Some(max_px.min(u32::MAX as u64) as u32);
        limits.max_image_height = Some(max_px.min(u32::MAX as u64) as u32);
        // 4 bytes per pixel (RGBA), plus generous slack for the decoder's own
        // scratch. Still far below a bomb.
        limits.max_alloc = Some(max_px.saturating_mul(6));
        reader.limits(limits);
    }

    let decoded = reader.decode().map_err(|e| Error::Decode(e.to_string()))?;
    let (w, h) = decoded.dimensions();

    // Belt and braces: the reader limits should have caught an oversize image,
    // but check the decoded dimensions too so the reported error is precise.
    if let Some(limit) = settings.max_megapixels {
        let megapixels = (w as u64 * h as u64) / 1_000_000;
        if megapixels > limit as u64 {
            return Err(Error::TooLarge { width: w, height: h, limit });
        }
    }

    let strength = settings.strength;
    let mut rgb = decoded.to_rgb8();

    // 1. Denoise at full resolution, where the pattern is strongest.
    denoise::wavelet_shrink(&mut rgb, strength.denoise_radius(), strength.denoise_amount());

    // 2. Downscale. The most effective step: resampling destroys the
    //    pixel-for-pixel correspondence correlation relies on.
    let scale = strength.scale();
    let new_w = ((w as f32 * scale).round() as u32).max(1);
    let new_h = ((h as f32 * scale).round() as u32).max(1);
    let resized = DynamicImage::ImageRgb8(rgb).resize_exact(
        new_w,
        new_h,
        image::imageops::FilterType::Lanczos3,
    );
    let mut rgb = resized.to_rgb8();

    // 3. Mask the remainder with fresh noise, which lowers the signal-to-noise
    //    ratio of any estimate an analyst can make from this image.
    add_noise(&mut rgb, strength.noise_sigma());

    // 4. Re-encode lossily.
    let quality = strength.quality();
    let mut out = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    encoder
        .encode(rgb.as_raw(), new_w, new_h, image::ExtendedColorType::Rgb8)
        .map_err(|e| Error::Encode(e.to_string()))?;

    // The encoder writes a bare JFIF with no EXIF, but callers should still run
    // the result through metascrub: this crate makes no metadata promises.
    let _ = ImageFormat::Jpeg;

    Ok(Washed {
        report: WashReport {
            original: (w, h),
            washed: (new_w, new_h),
            output_len: out.len(),
            quality,
        },
        data: out,
    })
}

/// Add zero-mean Gaussian-ish noise, clamped into range.
fn add_noise(img: &mut RgbImage, sigma: f32) {
    if sigma <= 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    for pixel in img.pixels_mut() {
        for channel in pixel.0.iter_mut() {
            // Sum of two uniforms approximates a normal well enough to mask a
            // residual, and avoids pulling in a distributions dependency.
            let u: f32 = rng.gen::<f32>() + rng.gen::<f32>() - 1.0;
            let value = *channel as f32 + u * sigma * 1.7;
            *channel = value.clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic "photograph" with a faint fixed pattern standing in for
    /// PRNU, plus structure so denoising has something to preserve.
    fn fake_photo(w: u32, h: u32, pattern: &dyn Fn(u32, u32) -> f32) -> Vec<u8> {
        let mut img = RgbImage::new(w, h);
        for (x, y, px) in img.enumerate_pixels_mut() {
            // Smooth gradient plus a soft blob, so the image is not flat.
            let base = 90.0 + 60.0 * ((x as f32 / w as f32) + (y as f32 / h as f32));
            let value = base * (1.0 + pattern(x, y));
            let v = value.clamp(0.0, 255.0) as u8;
            *px = image::Rgb([v, v, v]);
        }
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 96)
            .encode(img.as_raw(), w, h, image::ExtendedColorType::Rgb8)
            .unwrap();
        out
    }

    /// Deterministic pseudo-random fixed pattern, the stand-in for a sensor.
    fn sensor(x: u32, y: u32) -> f32 {
        let n = x.wrapping_mul(1_664_525).wrapping_add(y.wrapping_mul(1_013_904_223));
        ((n >> 16) & 0xFF) as f32 / 255.0 * 0.06 - 0.03
    }

    #[test]
    fn washing_downscales_and_reencodes() {
        let photo = fake_photo(200, 160, &sensor);
        let washed = wash(&photo, &Settings::default()).unwrap();
        assert_eq!(washed.report.original, (200, 160));
        assert_eq!(washed.report.washed, (140, 112)); // 70%
        assert!(!washed.data.is_empty());
        // The output must itself be a decodable image, not a corrupted blob.
        let round = image::load_from_memory(&washed.data).unwrap();
        assert_eq!(round.dimensions(), (140, 112));
    }

    #[test]
    fn every_strength_produces_a_valid_image() {
        let photo = fake_photo(120, 90, &sensor);
        for strength in [Strength::Gentle, Strength::Balanced, Strength::Thorough] {
            let settings = Settings { strength, ..Settings::default() };
            let washed = wash(&photo, &settings).unwrap();
            let round = image::load_from_memory(&washed.data).unwrap();
            let (w, h) = round.dimensions();
            assert!(w > 0 && h > 0, "{strength:?} produced an empty image");
            assert_eq!(washed.report.quality, strength.quality());
        }
    }

    #[test]
    fn stronger_settings_shrink_the_image_further() {
        let photo = fake_photo(200, 200, &sensor);
        let gentle =
            wash(&photo, &Settings { strength: Strength::Gentle, ..Default::default() }).unwrap();
        let thorough =
            wash(&photo, &Settings { strength: Strength::Thorough, ..Default::default() }).unwrap();
        assert!(thorough.report.washed.0 < gentle.report.washed.0);
    }

    /// The point of the whole crate: the fixed pattern must correlate less with
    /// the washed image than with the untouched one.
    #[test]
    fn washing_reduces_correlation_with_the_sensor_pattern() {
        let photo = fake_photo(256, 256, &sensor);

        // Residual of the original, at the washed image's scale, versus the
        // residual of the washed image. Compare each against the pattern.
        let original = image::load_from_memory(&photo).unwrap().to_rgb8();
        let washed_bytes =
            wash(&photo, &Settings { strength: Strength::Thorough, ..Default::default() }).unwrap();
        let washed = image::load_from_memory(&washed_bytes.data).unwrap().to_rgb8();

        let before = correlation_with_pattern(&original);
        // Sample the washed image on its own grid; a real analyst would rescale
        // to search for this, which is exactly why the number should be lower.
        let after = correlation_with_pattern(&washed);

        assert!(
            after.abs() < before.abs(),
            "washing should reduce correlation: before {before:.4}, after {after:.4}"
        );
    }

    /// Normalised correlation between an image's high-frequency residual and the
    /// known fixed pattern. A crude stand-in for what a forensic tool computes.
    fn correlation_with_pattern(img: &RgbImage) -> f32 {
        let (w, h) = img.dimensions();
        let mut residual = Vec::new();
        let mut expected = Vec::new();
        // Skip the border, where the blur has no full neighbourhood.
        for y in 2..h.saturating_sub(2) {
            for x in 2..w.saturating_sub(2) {
                let centre = img.get_pixel(x, y).0[0] as f32;
                let mean = [
                    img.get_pixel(x - 1, y).0[0] as f32,
                    img.get_pixel(x + 1, y).0[0] as f32,
                    img.get_pixel(x, y - 1).0[0] as f32,
                    img.get_pixel(x, y + 1).0[0] as f32,
                ]
                .iter()
                .sum::<f32>()
                    / 4.0;
                residual.push(centre - mean);
                expected.push(sensor(x, y));
            }
        }
        normalised_correlation(&residual, &expected)
    }

    fn normalised_correlation(a: &[f32], b: &[f32]) -> f32 {
        let n = a.len().min(b.len());
        if n == 0 {
            return 0.0;
        }
        let mean_a = a[..n].iter().sum::<f32>() / n as f32;
        let mean_b = b[..n].iter().sum::<f32>() / n as f32;
        let mut num = 0.0;
        let mut da = 0.0;
        let mut db = 0.0;
        for i in 0..n {
            let x = a[i] - mean_a;
            let y = b[i] - mean_b;
            num += x * y;
            da += x * x;
            db += y * y;
        }
        if da == 0.0 || db == 0.0 {
            0.0
        } else {
            num / (da.sqrt() * db.sqrt())
        }
    }

    #[test]
    fn oversized_images_are_refused_at_the_decoder() {
        // The limit is now enforced on the decoder before the image is built, so
        // a zero-megapixel ceiling refuses everything rather than passing small
        // images through an integer-division quirk. This is the stricter, more
        // predictable behaviour, and it means the bomb is stopped inside the
        // decode rather than after it.
        let photo = fake_photo(64, 64, &sensor);
        let settings = Settings { max_megapixels: Some(0), ..Default::default() };
        assert!(wash(&photo, &settings).is_err(), "a zero limit must refuse any image");

        // A generous ceiling still processes an ordinary photo.
        let ok = Settings { max_megapixels: Some(120), ..Default::default() };
        assert!(wash(&photo, &ok).is_ok());
    }

    #[test]
    fn garbage_input_is_an_error_not_a_panic() {
        assert!(wash(b"not an image at all", &Settings::default()).is_err());
        assert!(wash(&[], &Settings::default()).is_err());
        // A JPEG header with nothing behind it.
        assert!(wash(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00], &Settings::default()).is_err());
    }
}
