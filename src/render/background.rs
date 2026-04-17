use tiny_skia::{Pixmap, Color, Paint, FillRule, Transform, PathBuilder};
use crate::config::*;

/// Orb definition: position derived from trig functions at time t
struct Orb {
    color: [f32; 4],     // RGBA
    radius_x: f32,
    radius_y: f32,
    // Orbital parameters
    x_amp: f32,
    y_amp: f32,
    x_freq: f32,
    y_freq: f32,
    x_phase: f32,
    y_phase: f32,
}

const ORBS: [Orb; 3] = [
    // Gold — gentle drift
    Orb {
        color: [186.0 / 255.0, 117.0 / 255.0, 23.0 / 255.0, 0.08],
        radius_x: 500.0,
        radius_y: 400.0,
        x_amp: 0.20,
        y_amp: 0.15,
        x_freq: 0.06,
        y_freq: 0.045,
        x_phase: 0.0,
        y_phase: 0.0,
    },
    // Blue — slow orbit
    Orb {
        color: [60.0 / 255.0, 60.0 / 255.0, 180.0 / 255.0, 0.06],
        radius_x: 450.0,
        radius_y: 450.0,
        x_amp: 0.18,
        y_amp: 0.16,
        x_freq: 0.04,
        y_freq: 0.055,
        x_phase: 0.0,
        y_phase: 0.0,
    },
    // Purple — wide, slow arc
    Orb {
        color: [120.0 / 255.0, 60.0 / 255.0, 160.0 / 255.0, 0.05],
        radius_x: 420.0,
        radius_y: 350.0,
        x_amp: 0.22,
        y_amp: 0.12,
        x_freq: 0.035,
        y_freq: 0.07,
        x_phase: 2.0,
        y_phase: 1.0,
    },
];

/// Render the animated glow background at a given time (seconds).
/// Returns a WIDTH×HEIGHT RGBA pixmap. Orb radii scale with RESOLUTION_SCALE
/// so the look stays consistent when rendering at 4K.
pub fn render_background(time_sec: f64) -> Pixmap {
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).unwrap();

    pixmap.fill(Color::from_rgba8(BG_COLOR[0], BG_COLOR[1], BG_COLOR[2], BG_COLOR[3]));

    let t = time_sec as f32;

    for orb in &ORBS {
        let cx = (0.5 + orb.x_amp * (t * orb.x_freq + orb.x_phase).sin()) * WIDTH as f32;
        let cy = (0.5 + orb.y_amp * (t * orb.y_freq + orb.y_phase).cos()) * HEIGHT as f32;

        render_soft_ellipse(
            &mut pixmap,
            cx,
            cy,
            orb.radius_x * RESOLUTION_SCALE,
            orb.radius_y * RESOLUTION_SCALE,
            orb.color,
        );
    }

    pixmap
}

/// Render a soft radial gradient ellipse onto the pixmap.
/// Simulates CSS `radial-gradient(ellipse WxH at cx cy, color, transparent 70%)`
fn render_soft_ellipse(
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    color: [f32; 4],
) {
    let data = pixmap.data_mut();
    let width = WIDTH as usize;
    let height = HEIGHT as usize;

    // Only iterate over the bounding box of the ellipse
    let x_min = ((cx - rx * 1.5) as usize).max(0);
    let x_max = ((cx + rx * 1.5) as usize).min(width);
    let y_min = ((cy - ry * 1.5) as usize).max(0);
    let y_max = ((cy + ry * 1.5) as usize).min(height);

    for y in y_min..y_max {
        for x in x_min..x_max {
            // Normalized distance from center (elliptical)
            let dx = (x as f32 - cx) / rx;
            let dy = (y as f32 - cy) / ry;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist > 1.5 {
                continue;
            }

            // Fade to transparent at 70% of radius
            let alpha = if dist < 0.7 {
                color[3]
            } else {
                let t = (dist - 0.7) / 0.8; // 0.7 to 1.5
                color[3] * (1.0 - t).max(0.0)
            };

            if alpha < 0.001 {
                continue;
            }

            // Alpha-blend onto existing pixel
            let idx = (y * width + x) * 4;
            let dst_r = data[idx] as f32 / 255.0;
            let dst_g = data[idx + 1] as f32 / 255.0;
            let dst_b = data[idx + 2] as f32 / 255.0;

            let src_r = color[0] * alpha;
            let src_g = color[1] * alpha;
            let src_b = color[2] * alpha;

            data[idx] = ((dst_r * (1.0 - alpha) + src_r) * 255.0).min(255.0) as u8;
            data[idx + 1] = ((dst_g * (1.0 - alpha) + src_g) * 255.0).min(255.0) as u8;
            data[idx + 2] = ((dst_b * (1.0 - alpha) + src_b) * 255.0).min(255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_background() {
        let pixmap = render_background(0.0);
        assert_eq!(pixmap.width(), WIDTH);
        assert_eq!(pixmap.height(), HEIGHT);
        // Save for visual inspection
        pixmap.save_png("output/test_background.png").unwrap();
    }
}
