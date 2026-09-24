//! Anti-aliased rendering of the two stacked menu-bar usage bars.
//!
//! The image is drawn at 2x for Retina: 56×36 px, which macOS shows as 28×18 pt
//! (the status-item icon height). It is a colored, non-template image, so the
//! track uses a mid-grey that stays visible on light and dark menu bars.

/// Rendered image width in pixels.
pub const WIDTH: u32 = 56;
/// Rendered image height in pixels.
pub const HEIGHT: u32 = 36;

const BAR_HEIGHT: f64 = 13.0;
const BAR_GAP: f64 = 4.0;
const INSET_X: f64 = 2.0;

type Rgba = [f64; 4];

const TRACK: Rgba = [0.5, 0.5, 0.5, 0.45];
const CLAUDE: Rgba = [1.0, 138.0 / 255.0, 91.0 / 255.0, 1.0];
const CODEX: Rgba = [60.0 / 255.0, 242.0 / 255.0, 1.0, 1.0];
const WARNING: Rgba = [1.0, 176.0 / 255.0, 32.0 / 255.0, 1.0];
const CRITICAL: Rgba = [1.0, 45.0 / 255.0, 111.0 / 255.0, 1.0];

/// Renders straight-alpha RGBA pixels for Claude (top) and Codex (bottom)
/// weekly usage. `None` draws an empty track. Fills turn amber at 75% and
/// magenta-red at 90%, matching the app's thresholds.
pub fn render(claude: Option<u8>, codex: Option<u8>) -> Vec<u8> {
    let top = (f64::from(HEIGHT) - 2.0 * BAR_HEIGHT - BAR_GAP) / 2.0;
    let bars = [
        (top, claude, CLAUDE),
        (top + BAR_HEIGHT + BAR_GAP, codex, CODEX),
    ];
    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let (px, py) = (f64::from(x), f64::from(y));
            let mut color = [0.0; 4];
            for (bar_top, used, accent) in bars {
                let shape = capsule_coverage(px + 0.5, py + 0.5, bar_top);
                if shape <= 0.0 {
                    continue;
                }
                color = over(with_alpha(TRACK, shape), color);
                if let Some(used) = used {
                    let edge = INSET_X
                        + (f64::from(WIDTH) - 2.0 * INSET_X) * f64::from(used.min(100)) / 100.0;
                    let fill = shape * (edge - px).clamp(0.0, 1.0);
                    if fill > 0.0 {
                        color = over(with_alpha(fill_color(used, accent), fill), color);
                    }
                }
            }
            pixels.extend(color.map(to_byte));
        }
    }
    pixels
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to 0–255 immediately before the cast"
)]
fn to_byte(channel: f64) -> u8 {
    (channel * 255.0).round().clamp(0.0, 255.0) as u8
}

fn fill_color(used: u8, accent: Rgba) -> Rgba {
    if used >= 90 {
        CRITICAL
    } else if used >= 75 {
        WARNING
    } else {
        accent
    }
}

/// Pixel coverage of a horizontal capsule whose top edge is `bar_top`.
fn capsule_coverage(x: f64, y: f64, bar_top: f64) -> f64 {
    let radius = BAR_HEIGHT / 2.0;
    let center_y = bar_top + radius;
    let start = INSET_X + radius;
    let end = f64::from(WIDTH) - INSET_X - radius;
    let dx = x - x.clamp(start, end);
    let distance = dx.hypot(y - center_y) - radius;
    (0.5 - distance).clamp(0.0, 1.0)
}

fn with_alpha(color: Rgba, coverage: f64) -> Rgba {
    [color[0], color[1], color[2], color[3] * coverage]
}

/// Straight-alpha source-over compositing.
fn over(source: Rgba, destination: Rgba) -> Rgba {
    let alpha = source[3] + destination[3] * (1.0 - source[3]);
    if alpha <= 0.0 {
        return [0.0; 4];
    }
    let blend = |index: usize| {
        (source[index] * source[3] + destination[index] * destination[3] * (1.0 - source[3]))
            / alpha
    };
    [blend(0), blend(1), blend(2), alpha]
}

#[cfg(test)]
mod tests {
    use super::{HEIGHT, WIDTH, render};

    fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
        let index = ((y * WIDTH + x) * 4) as usize;
        [
            pixels[index],
            pixels[index + 1],
            pixels[index + 2],
            pixels[index + 3],
        ]
    }

    const TOP_ROW: u32 = 10;
    const BOTTOM_ROW: u32 = 26;

    #[test]
    fn image_has_the_retina_status_item_size() {
        assert_eq!(render(None, None).len(), (WIDTH * HEIGHT * 4) as usize);
    }

    #[test]
    fn claude_is_coral_on_top_and_codex_cyan_below() {
        let pixels = render(Some(50), Some(50));
        assert_eq!(pixel(&pixels, 12, TOP_ROW), [255, 138, 91, 255]);
        assert_eq!(pixel(&pixels, 12, BOTTOM_ROW), [60, 242, 255, 255]);
        // Past the 50% edge only the translucent grey track remains.
        let track = pixel(&pixels, 44, TOP_ROW);
        assert_eq!(track[..3], [128, 128, 128]);
        assert!(track[3] > 80 && track[3] < 150);
        // Corners and the gap between bars are transparent.
        assert_eq!(pixel(&pixels, 0, 0)[3], 0);
        assert_eq!(pixel(&pixels, 28, HEIGHT / 2)[3], 0);
    }

    #[test]
    fn fills_follow_warning_and_critical_thresholds() {
        let pixels = render(Some(80), Some(95));
        assert_eq!(pixel(&pixels, 12, TOP_ROW), [255, 176, 32, 255]);
        assert_eq!(pixel(&pixels, 12, BOTTOM_ROW), [255, 45, 111, 255]);
    }

    #[test]
    fn missing_or_zero_usage_draws_only_the_track() {
        for pixels in [render(None, None), render(Some(0), Some(0))] {
            assert_eq!(pixel(&pixels, 12, TOP_ROW)[..3], [128, 128, 128]);
            assert_eq!(pixel(&pixels, 12, BOTTOM_ROW)[..3], [128, 128, 128]);
        }
    }
}
