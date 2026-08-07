//! Denoising, as the inverse of what a forensic tool does.
//!
//! PRNU detection works by estimating the noise-free image, subtracting it to
//! get a residual, and correlating that residual against a reference pattern.
//! So the counter-operation is the same first two steps, followed by keeping
//! the estimate and discarding most of the residual instead of the other way
//! around.
//!
//! This is single-scale wavelet shrinkage in everything but name: separate the
//! image into a smooth part and a detail part, shrink the small detail values
//! (which are mostly noise) toward zero, and add back what remains (which is
//! mostly real edges). Doing it at one scale with a box blur rather than a full
//! wavelet transform costs some fidelity and saves a great deal of complexity,
//! and complexity in a routine that runs over files from strangers is a cost of
//! its own.

use image::RgbImage;

/// Shrink a value toward zero by `amount`, leaving larger values mostly intact.
///
/// Below the threshold, values are assumed to be noise and are strongly
/// suppressed. Above it, they are assumed to be real detail and are kept, less
/// a constant, so there is no discontinuity at the threshold that would show up
/// as a visible edge artefact.
pub fn soft_threshold(value: f32, threshold: f32) -> f32 {
    if value.abs() <= threshold {
        0.0
    } else if value > 0.0 {
        value - threshold
    } else {
        value + threshold
    }
}

/// Denoise in place.
///
/// `radius` sets the scale of detail treated as noise. `amount` between 0 and 1
/// sets how much of that detail is removed.
pub fn wavelet_shrink(img: &mut RgbImage, radius: f32, amount: f32) {
    if radius <= 0.0 || amount <= 0.0 {
        return;
    }
    let (w, h) = img.dimensions();
    if w < 3 || h < 3 {
        return;
    }

    let radius_px = radius.round().max(1.0) as u32;
    let amount = amount.clamp(0.0, 1.0);

    // Work per channel so a colour cast is never introduced.
    for channel in 0..3usize {
        let plane: Vec<f32> = img.pixels().map(|p| p.0[channel] as f32).collect();
        let smooth = box_blur(&plane, w, h, radius_px);

        // Threshold scaled to the actual residual energy, so a noisy image is
        // cleaned harder than a clean one rather than by a fixed amount.
        let mut sum_sq = 0.0f64;
        for (a, b) in plane.iter().zip(smooth.iter()) {
            let d = (a - b) as f64;
            sum_sq += d * d;
        }
        let rms = (sum_sq / plane.len() as f64).sqrt() as f32;
        let threshold = rms * amount * 2.0;

        for (i, px) in img.pixels_mut().enumerate() {
            let detail = plane[i] - smooth[i];
            let kept = soft_threshold(detail, threshold);
            // Blend rather than replace, so `amount` scales smoothly and the
            // strongest setting still leaves a little texture.
            let denoised = smooth[i] + kept;
            let value = plane[i] * (1.0 - amount) + denoised * amount;
            px.0[channel] = value.clamp(0.0, 255.0) as u8;
        }
    }
}

/// Separable box blur. Two passes of a box approximate a Gaussian closely
/// enough for estimating the smooth component, at a fraction of the cost.
fn box_blur(src: &[f32], w: u32, h: u32, radius: u32) -> Vec<f32> {
    let pass = box_blur_h(src, w, h, radius);
    let pass = box_blur_v(&pass, w, h, radius);
    let pass = box_blur_h(&pass, w, h, radius);
    box_blur_v(&pass, w, h, radius)
}

fn box_blur_h(src: &[f32], w: u32, h: u32, radius: u32) -> Vec<f32> {
    let mut out = vec![0.0f32; src.len()];
    let w = w as i64;
    let r = radius as i64;
    for y in 0..h as i64 {
        let row = y * w;
        for x in 0..w {
            let mut sum = 0.0;
            let mut count = 0.0;
            for dx in -r..=r {
                // Clamp at the edges rather than wrapping, which would fold one
                // side of the picture into the other.
                let sx = (x + dx).clamp(0, w - 1);
                sum += src[(row + sx) as usize];
                count += 1.0;
            }
            out[(row + x) as usize] = sum / count;
        }
    }
    out
}

fn box_blur_v(src: &[f32], w: u32, h: u32, radius: u32) -> Vec<f32> {
    let mut out = vec![0.0f32; src.len()];
    let (wi, hi) = (w as i64, h as i64);
    let r = radius as i64;
    for x in 0..wi {
        for y in 0..hi {
            let mut sum = 0.0;
            let mut count = 0.0;
            for dy in -r..=r {
                let sy = (y + dy).clamp(0, hi - 1);
                sum += src[(sy * wi + x) as usize];
                count += 1.0;
            }
            out[(y * wi + x) as usize] = sum / count;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_threshold_kills_small_values_and_keeps_large_ones() {
        assert_eq!(soft_threshold(0.5, 1.0), 0.0);
        assert_eq!(soft_threshold(-0.5, 1.0), 0.0);
        assert_eq!(soft_threshold(3.0, 1.0), 2.0);
        assert_eq!(soft_threshold(-3.0, 1.0), -2.0);
        // Continuous at the threshold: no visible step in the output.
        assert!((soft_threshold(1.001, 1.0) - 0.0).abs() < 0.01);
    }

    #[test]
    fn blur_of_a_flat_plane_is_the_same_plane() {
        let plane = vec![128.0f32; 64];
        let out = box_blur(&plane, 8, 8, 2);
        for v in out {
            assert!((v - 128.0).abs() < 0.01);
        }
    }

    #[test]
    fn blur_clamps_at_edges_rather_than_wrapping() {
        // A bright left column and dark right column. If the blur wrapped, the
        // bright edge would bleed into the dark one.
        let (w, h) = (8u32, 4u32);
        let mut plane = vec![0.0f32; (w * h) as usize];
        for y in 0..h {
            plane[(y * w) as usize] = 255.0;
        }
        let out = box_blur(&plane, w, h, 1);
        for y in 0..h {
            let right = out[(y * w + w - 1) as usize];
            assert!(right < 1.0, "brightness wrapped around to the far edge: {right}");
        }
    }

    #[test]
    fn denoising_reduces_high_frequency_energy() {
        let (w, h) = (48u32, 48u32);
        let mut img = RgbImage::new(w, h);
        // Smooth ramp plus alternating per-pixel noise.
        for (x, y, px) in img.enumerate_pixels_mut() {
            let base = 100.0 + (x as f32) * 1.5;
            let jitter = if (x + y) % 2 == 0 { 9.0 } else { -9.0 };
            let v = (base + jitter).clamp(0.0, 255.0) as u8;
            *px = image::Rgb([v, v, v]);
        }
        let before = high_frequency_energy(&img);
        wavelet_shrink(&mut img, 1.5, 0.9);
        let after = high_frequency_energy(&img);
        assert!(after < before, "denoise should reduce detail energy: {before} -> {after}");
    }

    #[test]
    fn denoising_is_a_no_op_at_zero_amount() {
        let mut img = RgbImage::new(16, 16);
        for (x, _, px) in img.enumerate_pixels_mut() {
            *px = image::Rgb([(x * 8) as u8, 40, 200]);
        }
        let before = img.clone();
        wavelet_shrink(&mut img, 1.0, 0.0);
        assert_eq!(before, img);
    }

    #[test]
    fn tiny_images_do_not_panic() {
        for (w, h) in [(1u32, 1u32), (2, 2), (1, 9), (9, 1)] {
            let mut img = RgbImage::new(w, h);
            wavelet_shrink(&mut img, 2.0, 0.8);
        }
    }

    fn high_frequency_energy(img: &RgbImage) -> f64 {
        let (w, h) = img.dimensions();
        let mut total = 0.0;
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let c = img.get_pixel(x, y).0[0] as f64;
                let n = (img.get_pixel(x - 1, y).0[0] as f64
                    + img.get_pixel(x + 1, y).0[0] as f64
                    + img.get_pixel(x, y - 1).0[0] as f64
                    + img.get_pixel(x, y + 1).0[0] as f64)
                    / 4.0;
                total += (c - n).abs();
            }
        }
        total
    }
}
