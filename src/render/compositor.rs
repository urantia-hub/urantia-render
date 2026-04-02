use tiny_skia::Pixmap;

/// Apply a global opacity to all non-background pixels (for fade in/out).
/// Modifies the pixmap's alpha channel in place.
pub fn apply_opacity(pixmap: &mut Pixmap, opacity: f32) {
    if (opacity - 1.0).abs() < 0.001 {
        return; // no-op at full opacity
    }

    let data = pixmap.data_mut();
    for i in (3..data.len()).step_by(4) {
        data[i] = (data[i] as f32 * opacity) as u8;
    }
}

/// Composite a foreground pixmap onto a background pixmap with a given opacity.
/// The foreground is blended on top of the background using alpha compositing.
pub fn composite(bg: &mut Pixmap, fg: &Pixmap, opacity: f32) {
    let bg_data = bg.data_mut();
    let fg_data = fg.data();

    for i in (0..bg_data.len()).step_by(4) {
        let fa = fg_data[i + 3] as f32 / 255.0 * opacity;
        if fa < 0.001 {
            continue;
        }

        let inv_a = 1.0 - fa;

        bg_data[i] = (bg_data[i] as f32 * inv_a + fg_data[i] as f32 * fa) as u8;
        bg_data[i + 1] = (bg_data[i + 1] as f32 * inv_a + fg_data[i + 1] as f32 * fa) as u8;
        bg_data[i + 2] = (bg_data[i + 2] as f32 * inv_a + fg_data[i + 2] as f32 * fa) as u8;
        bg_data[i + 3] = 255;
    }
}
