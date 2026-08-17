//! The window icon, rasterised from the same shapes the mark is drawn from.
//!
//! A desktop file plus an installed icon theme covers the launcher, but only
//! once something has been installed. The application is also meant to work as
//! a single downloaded file that nobody has installed anywhere, and in that
//! case the only icon a window manager can find is the one the window hands it.
//! Without this the title bar, the task switcher and the dock all show a blank
//! placeholder.
//!
//! Rasterised here rather than embedded as a PNG for the same reason
//! [`crate::draw_crake`] exists: the shapes are a handful of circles, a
//! triangle and two capsules, which is far less code than an image decoder and
//! cannot go stale against a binary blob nobody re-exports.
//!
//! `packaging/linux/make-icons.py` renders the installed theme icons from the
//! same coordinates, so the two agree by construction.

/// Side length of the icon handed to the window system. 64 is the largest size
/// a task switcher normally asks for, and scaling one good raster down beats
/// giving the compositor a small one to scale up.
const SIZE: usize = 64;

/// Samples per axis. Nine samples a pixel is enough to keep the beak and the
/// legs from going ragged, and it runs once at startup.
const SS: usize = 3;

/// The mark's own coordinate space, from `brand/metascrub.svg`.
const VIEW: (f32, f32, f32, f32) = (14.0, 15.0, 74.0, 78.0);
const TEAL: (u8, u8, u8) = (0x5f, 0xb0, 0xba);

fn in_circle(x: f32, y: f32, cx: f32, cy: f32, r: f32) -> bool {
    (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r
}

fn in_ellipse(x: f32, y: f32, cx: f32, cy: f32, rx: f32, ry: f32) -> bool {
    let (dx, dy) = ((x - cx) / rx, (y - cy) / ry);
    dx * dx + dy * dy <= 1.0
}

/// A stroked line with round caps, which is what each leg is.
fn in_capsule(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, half: f32) -> bool {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len2 = dx * dx + dy * dy;
    let t =
        if len2 == 0.0 { 0.0 } else { (((x - x0) * dx + (y - y0) * dy) / len2).clamp(0.0, 1.0) };
    let (px, py) = (x0 + t * dx, y0 + t * dy);
    (x - px) * (x - px) + (y - py) * (y - py) <= half * half
}

fn in_triangle(x: f32, y: f32, t: [(f32, f32); 3]) -> bool {
    let sign = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| {
        (a.0 - c.0) * (b.1 - c.1) - (b.0 - c.0) * (a.1 - c.1)
    };
    let p = (x, y);
    let d1 = sign(p, t[0], t[1]);
    let d2 = sign(p, t[1], t[2]);
    let d3 = sign(p, t[2], t[0]);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

/// True where the mark is inked. Mirrors the mask in `brand/metascrub.svg`.
///
/// The beak in the SVG is a curve. Approximating it with a triangle costs a
/// fraction of a pixel at this size and saves carrying a bezier flattener.
fn covered(x: f32, y: f32) -> bool {
    if in_circle(x, y, 50.0, 30.0, 3.6) {
        return false; // the eye is cut back out of the shape
    }
    in_circle(x, y, 54.0, 54.0, 19.0)                                   // body
        || in_triangle(x, y, [(70.0, 49.0), (80.0, 46.0), (72.0, 58.0)]) // tail
        || in_circle(x, y, 47.0, 33.0, 13.0)                             // head
        || in_triangle(x, y, [(38.0, 30.0), (19.0, 37.0), (38.0, 34.0)]) // beak
        || in_capsule(x, y, 49.0, 70.0, 49.0, 80.0, 1.3)                 // legs
        || in_capsule(x, y, 58.0, 70.0, 58.0, 80.0, 1.3)
        || in_ellipse(x, y, 54.0, 83.0, 22.0, 3.2) // ground
}

/// The icon as straight (non-premultiplied) RGBA, which is what eframe wants.
pub fn window_icon() -> egui::IconData {
    let (vx, vy, vw, vh) = VIEW;
    let size = SIZE as f32;
    // Fit the mark's box into the square without distorting it, with a margin
    // so it is not flush against the edge of a launcher tile.
    let scale = (size * 0.88) / vw.max(vh);
    let ox = (size - vw * scale) / 2.0;
    let oy = (size - vh * scale) / 2.0;

    let step = 1.0 / SS as f32;
    let weight = 1.0 / (SS * SS) as f32;
    let mut rgba = Vec::with_capacity(SIZE * SIZE * 4);
    for py in 0..SIZE {
        for px in 0..SIZE {
            let mut hits = 0u32;
            for sy in 0..SS {
                let yy = (py as f32 + (sy as f32 + 0.5) * step - oy) / scale + vy;
                for sx in 0..SS {
                    let xx = (px as f32 + (sx as f32 + 0.5) * step - ox) / scale + vx;
                    if covered(xx, yy) {
                        hits += 1;
                    }
                }
            }
            let alpha = (hits as f32 * weight * 255.0).round() as u8;
            rgba.extend_from_slice(&[TEAL.0, TEAL.1, TEAL.2, alpha]);
        }
    }
    egui::IconData { rgba, width: SIZE as u32, height: SIZE as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_is_the_size_it_claims() {
        let icon = window_icon();
        assert_eq!(icon.width, SIZE as u32);
        assert_eq!(icon.height, SIZE as u32);
        assert_eq!(icon.rgba.len(), SIZE * SIZE * 4);
    }

    #[test]
    fn icon_has_a_bird_in_it_and_air_around_it() {
        let icon = window_icon();
        let alpha: Vec<u8> = icon.rgba.chunks(4).map(|p| p[3]).collect();
        let inked = alpha.iter().filter(|&&a| a > 128).count();
        // A mark that fills everything or nothing means the transform is wrong,
        // which is exactly the failure a blank icon would hide.
        assert!(inked > alpha.len() / 8, "icon is nearly empty: {inked} of {}", alpha.len());
        assert!(inked < alpha.len() * 3 / 4, "icon is nearly solid: {inked} of {}", alpha.len());
        // The corners must stay clear, or it is a filled square rather than a bird.
        assert_eq!(alpha[0], 0, "top-left corner is not transparent");
        assert_eq!(alpha[SIZE - 1], 0, "top-right corner is not transparent");
    }

    #[test]
    fn the_eye_is_a_hole_not_a_dot() {
        // Straight through the middle of the eye, in the mark's own coordinates.
        assert!(!covered(50.0, 30.0), "the eye should be cut out of the head");
        assert!(covered(47.0, 33.0 + 8.0), "the head should be inked below the eye");
    }
}
