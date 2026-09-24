//! Anti-aliased rendering of the two stacked menu-bar usage bars.
//!
//! The image is drawn at 2x for Retina: 56×36 px, which macOS shows as 28×18 pt
//! (the status-item icon height). It is a colored, non-template image, so the
//! track uses a mid-grey that stays visible on light and dark menu bars.
//!
//! Each bar carries its percentage centered in small stroked digits. Over the
//! track they follow the menu-bar appearance (dark on light, white on dark);
//! over the bright fills they are always dark, which stays readable on coral,
//! cyan, amber, and magenta-red alike.

/// Rendered image width in pixels.
pub const WIDTH: u32 = 56;
/// Rendered image height in pixels.
pub const HEIGHT: u32 = 36;

const BAR_HEIGHT: f64 = 15.0;
const BAR_GAP: f64 = 3.0;
const INSET_X: f64 = 2.0;

/// Digit box in pixels; strokes are centered on the glyph paths.
const DIGIT_WIDTH: f64 = 5.0;
const DIGIT_HEIGHT: f64 = 9.0;
const DIGIT_STROKE: f64 = 1.5;
/// Space between neighbouring digits' ink.
const DIGIT_SPACING: f64 = 1.3;

type Rgba = [f64; 4];

const TRACK: Rgba = [0.5, 0.5, 0.5, 0.45];
const CLAUDE: Rgba = [1.0, 138.0 / 255.0, 91.0 / 255.0, 1.0];
const CODEX: Rgba = [60.0 / 255.0, 242.0 / 255.0, 1.0, 1.0];
const WARNING: Rgba = [1.0, 176.0 / 255.0, 32.0 / 255.0, 1.0];
const CRITICAL: Rgba = [1.0, 45.0 / 255.0, 111.0 / 255.0, 1.0];
const TEXT_DARK: Rgba = [0.0, 0.0, 0.0, 0.8];
const TEXT_LIGHT: Rgba = [1.0, 1.0, 1.0, 0.92];

/// Appearance of the menu bar the image is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuBar {
    Light,
    Dark,
}

/// Renders straight-alpha RGBA pixels for Claude (top) and Codex (bottom)
/// weekly usage. `None` draws an empty, unlabeled track. Fills turn amber at
/// 75% and magenta-red at 90%, matching the app's thresholds.
pub fn render(claude: Option<u8>, codex: Option<u8>, menu_bar: MenuBar) -> Vec<u8> {
    let text_on_track = match menu_bar {
        MenuBar::Light => TEXT_DARK,
        MenuBar::Dark => TEXT_LIGHT,
    };
    let top = (f64::from(HEIGHT) - 2.0 * BAR_HEIGHT - BAR_GAP) / 2.0;
    let bars = [
        (top, claude.map(|used| used.min(100)), CLAUDE),
        (
            top + BAR_HEIGHT + BAR_GAP,
            codex.map(|used| used.min(100)),
            CODEX,
        ),
    ]
    .map(|(bar_top, used, accent)| {
        let label = used.map_or_else(Vec::new, |used| label_strokes(used, bar_top));
        (bar_top, used, accent, label)
    });
    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let (px, py) = (f64::from(x), f64::from(y));
            let mut color = [0.0; 4];
            for (bar_top, used, accent, label) in &bars {
                let shape = capsule_coverage(px + 0.5, py + 0.5, *bar_top);
                if shape <= 0.0 {
                    continue;
                }
                color = over(with_alpha(TRACK, shape), color);
                let Some(used) = *used else { continue };
                let edge = INSET_X + (f64::from(WIDTH) - 2.0 * INSET_X) * f64::from(used) / 100.0;
                // Fraction of this pixel column that lies under the fill.
                let filled = (edge - px).clamp(0.0, 1.0);
                if filled > 0.0 {
                    color = over(with_alpha(fill_color(used, *accent), shape * filled), color);
                }
                let ink = stroke_coverage(px + 0.5, py + 0.5, label);
                if ink > 0.0 {
                    let text = mix(text_on_track, TEXT_DARK, filled);
                    color = over(with_alpha(text, ink), color);
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

/// Line segments of `used` as centered digits inside the bar at `bar_top`.
fn label_strokes(used: u8, bar_top: f64) -> Vec<Segment> {
    let digits = used.to_string();
    let count = match used {
        0..=9 => 1.0,
        10..=99 => 2.0,
        _ => 3.0,
    };
    let ink_width = count * (DIGIT_WIDTH + DIGIT_STROKE) + (count - 1.0) * DIGIT_SPACING;
    let mut left = (f64::from(WIDTH) - ink_width) / 2.0 + DIGIT_STROKE / 2.0;
    let top = bar_top + (BAR_HEIGHT - DIGIT_HEIGHT) / 2.0;
    let mut segments = Vec::new();
    for digit in digits.bytes().map(|byte| byte - b'0') {
        for path in glyph(digit) {
            segments.extend(path.windows(2).map(|pair| {
                let place = |(x, y): Point| (left + x * DIGIT_WIDTH, top + y * DIGIT_HEIGHT);
                (place(pair[0]), place(pair[1]))
            }));
        }
        left += DIGIT_WIDTH + DIGIT_STROKE + DIGIT_SPACING;
    }
    segments
}

type Point = (f64, f64);
type Segment = (Point, Point);

/// Stroke paths of a digit in a unit box (x right, y down).
fn glyph(digit: u8) -> Vec<Vec<Point>> {
    match digit {
        0 => vec![arc((0.5, 0.5), (0.5, 0.5), 0.0, 360.0)],
        1 => vec![vec![(0.2, 0.2), (0.55, 0.0), (0.55, 1.0)]],
        2 => {
            let mut path = arc((0.5, 0.27), (0.5, 0.27), 180.0, 395.0);
            path.extend([(0.0, 1.0), (1.0, 1.0)]);
            vec![path]
        }
        3 => vec![
            arc((0.5, 0.25), (0.45, 0.25), 200.0, 450.0),
            arc((0.5, 0.75), (0.5, 0.25), 270.0, 520.0),
        ],
        4 => vec![vec![(0.75, 1.0), (0.75, 0.0), (0.0, 0.7), (1.0, 0.7)]],
        5 => vec![
            vec![(0.92, 0.0), (0.15, 0.0), (0.12, 0.45)],
            arc((0.48, 0.68), (0.5, 0.32), 235.0, 510.0),
        ],
        6 => vec![
            arc((0.5, 0.68), (0.5, 0.32), 0.0, 360.0),
            arc((1.0, 0.68), (1.0, 0.66), 180.0, 260.0),
        ],
        7 => vec![vec![(0.0, 0.0), (1.0, 0.0), (0.35, 1.0)]],
        8 => vec![
            arc((0.5, 0.24), (0.42, 0.24), 0.0, 360.0),
            arc((0.5, 0.72), (0.5, 0.28), 0.0, 360.0),
        ],
        // A 6 turned half a turn.
        _ => glyph(6)
            .into_iter()
            .map(|path| path.into_iter().map(|(x, y)| (1.0 - x, 1.0 - y)).collect())
            .collect(),
    }
}

/// An elliptical arc, clockwise on screen from `start` to `end` degrees
/// (0° points right, 90° points down), flattened into short segments.
fn arc(center: Point, radius: Point, start: f64, end: f64) -> Vec<Point> {
    const STEPS: u32 = 24;
    (0..=STEPS)
        .map(|step| {
            let angle = (start + (end - start) * f64::from(step) / f64::from(STEPS)).to_radians();
            (
                center.0 + radius.0 * angle.cos(),
                center.1 + radius.1 * angle.sin(),
            )
        })
        .collect()
}

/// Anti-aliased coverage of round-capped strokes along `segments`.
fn stroke_coverage(x: f64, y: f64, segments: &[Segment]) -> f64 {
    let distance = segments
        .iter()
        .map(|&(start, end)| segment_distance((x, y), start, end))
        .fold(f64::INFINITY, f64::min);
    (DIGIT_STROKE / 2.0 + 0.5 - distance).clamp(0.0, 1.0)
}

fn segment_distance(point: Point, start: Point, end: Point) -> f64 {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared == 0.0 {
        0.0
    } else {
        (((point.0 - start.0) * dx + (point.1 - start.1) * dy) / length_squared).clamp(0.0, 1.0)
    };
    (point.0 - start.0 - t * dx).hypot(point.1 - start.1 - t * dy)
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

fn mix(from: Rgba, to: Rgba, amount: f64) -> Rgba {
    [0, 1, 2, 3].map(|index| from[index] + (to[index] - from[index]) * amount)
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
    use super::{HEIGHT, MenuBar, WIDTH, render};

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
    /// On the left stroke of a lone centered "0" in the top bar.
    const ZERO_INK: (u32, u32) = (25, 8);

    #[test]
    fn image_has_the_retina_status_item_size() {
        assert_eq!(
            render(None, None, MenuBar::Light).len(),
            (WIDTH * HEIGHT * 4) as usize
        );
    }

    #[test]
    fn claude_is_coral_on_top_and_codex_cyan_below() {
        let pixels = render(Some(50), Some(50), MenuBar::Light);
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
        let pixels = render(Some(80), Some(95), MenuBar::Light);
        assert_eq!(pixel(&pixels, 12, TOP_ROW), [255, 176, 32, 255]);
        assert_eq!(pixel(&pixels, 12, BOTTOM_ROW), [255, 45, 111, 255]);
    }

    #[test]
    fn missing_or_zero_usage_leaves_the_track_unfilled() {
        for pixels in [
            render(None, None, MenuBar::Light),
            render(Some(0), Some(0), MenuBar::Light),
        ] {
            assert_eq!(pixel(&pixels, 12, TOP_ROW)[..3], [128, 128, 128]);
            assert_eq!(pixel(&pixels, 12, BOTTOM_ROW)[..3], [128, 128, 128]);
        }
        // Missing usage has no label at all.
        let (x, y) = ZERO_INK;
        assert_eq!(
            pixel(&render(None, None, MenuBar::Light), x, y)[..3],
            [128, 128, 128]
        );
    }

    #[test]
    fn digits_over_the_track_follow_the_menu_bar() {
        let (x, y) = ZERO_INK;
        let light = pixel(&render(Some(0), None, MenuBar::Light), x, y);
        let dark = pixel(&render(Some(0), None, MenuBar::Dark), x, y);
        assert!(
            light[0] < 40 && light[3] > 200,
            "dark ink expected, got {light:?}"
        );
        assert!(
            dark[0] > 220 && dark[3] > 200,
            "light ink expected, got {dark:?}"
        );
    }

    #[test]
    fn digits_over_the_fill_are_dark_and_centered() {
        for menu_bar in [MenuBar::Light, MenuBar::Dark] {
            let pixels = render(Some(100), None, menu_bar);
            // Columns of the top bar holding dark ink over the full fill.
            let inked = (0..WIDTH)
                .filter(|&x| (4..14).any(|y| pixel(&pixels, x, y)[0] < 100))
                .collect::<Vec<_>>();
            let (first, last) = (inked[0], inked[inked.len() - 1]);
            assert!(
                last - first >= 16,
                "three digits expected, got {first}..={last}"
            );
            assert!(
                (first + last).abs_diff(WIDTH) <= 2,
                "off-center {first}..={last}"
            );
            // Away from the digits the fill is untouched.
            assert_eq!(pixel(&pixels, 8, TOP_ROW), [255, 45, 111, 255]);
        }
    }
}
