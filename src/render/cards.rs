use tiny_skia::Pixmap;
use crate::config::*;
use crate::render::text::{TextRenderer, TextStyle};

/// Render a thumbnail image with 2.5x larger text for YouTube thumbnails.
/// Measures total text height to vertically center the label + title block.
pub fn render_thumbnail(
    renderer: &mut TextRenderer,
    pixmap: &mut Pixmap,
    paper_id: &str,
    paper_title: &str,
) {
    let h = HEIGHT as f32;

    let label = if paper_id == "0" {
        "Foreword".to_string()
    } else {
        format!("Paper {}", paper_id)
    };

    // Measure heights to vertically center the whole block
    let label_height = renderer.measure_text(&label, &TextStyle::thumbnail_label(0.0));
    let title_height = renderer.measure_text(paper_title, &TextStyle::thumbnail_title(0.0));
    let gap = 30.0; // space between label and title
    let total_height = label_height + gap + title_height;

    let start_y = (h - total_height) / 2.0;

    let label_style = TextStyle::thumbnail_label(start_y);
    renderer.render_text(pixmap, &label, &label_style);

    let title_style = TextStyle::thumbnail_title(start_y + label_height + gap);
    renderer.render_text(pixmap, paper_title, &title_style);
}

/// Render intro card — "PAPER 1" label + title, centered on screen.
pub fn render_intro_card(
    renderer: &mut TextRenderer,
    pixmap: &mut Pixmap,
    paper_id: &str,
    paper_title: &str,
) {
    let h = HEIGHT as f32;
    let center_y = h / 2.0;

    let label = if paper_id == "0" {
        "Foreword".to_string()
    } else {
        format!("Paper {}", paper_id)
    };

    let label_style = TextStyle::paper_label(center_y - 48.0);
    renderer.render_text(pixmap, &label, &label_style);

    let title_style = TextStyle::paper_title(center_y - 12.0);
    renderer.render_text(pixmap, paper_title, &title_style);
}

/// Render section card — section title centered on screen
pub fn render_section_card(
    renderer: &mut TextRenderer,
    pixmap: &mut Pixmap,
    section_title: &str,
) {
    let h = HEIGHT as f32;
    let style = TextStyle::section_title(h / 2.0 - 30.0);
    renderer.render_text(pixmap, section_title, &style);
}

/// Render outro card — logo icon + "UrantiaHub" + subtitle centered.
/// If tagline is provided, it replaces "urantiahub.com".
pub fn render_outro_card(
    renderer: &mut TextRenderer,
    pixmap: &mut Pixmap,
    tagline: Option<&str>,
) {
    let w = WIDTH as f32;
    let h = HEIGHT as f32;
    let center_y = h / 2.0;

    // Concentric circles logo above the text
    let logo_radius = 56.0;
    let logo_cx = w / 2.0;
    let logo_cy = center_y - 65.0;
    render_concentric_logo(pixmap, logo_cx, logo_cy, logo_radius);

    // "Urantia" (Lato Light) + "Hub" (Lato Bold) side by side
    let text_y = logo_cy + logo_radius + 15.0;

    let urantia_width = renderer.measure_text_width("Urantia", &TextStyle::outro_logo_light(0.0, 0.0));
    let hub_width = renderer.measure_text_width("Hub", &TextStyle::outro_logo_bold(0.0, 0.0));
    let total_width = urantia_width + hub_width;
    let text_x = (w - total_width) / 2.0;

    let light_style = TextStyle::outro_logo_light(text_x, text_y);
    renderer.render_text(pixmap, "Urantia", &light_style);

    // Light weight sits higher — nudge "Hub" up to align baselines
    let bold_style = TextStyle::outro_logo_bold(text_x + urantia_width, text_y - 5.0);
    renderer.render_text(pixmap, "Hub", &bold_style);

    // Subtitle below
    let subtitle_text = tagline.unwrap_or("urantiahub.com");
    let subtitle_style = TextStyle::outro_subtitle(text_y + 62.0);
    renderer.render_text(pixmap, subtitle_text, &subtitle_style);
}

/// Render the UrantiaHub concentric circles logo.
/// 7 circles: 6 stroked rings (increasing opacity) + 1 filled center dot.
fn render_concentric_logo(pixmap: &mut Pixmap, cx: f32, cy: f32, scale: f32) {
    // Original SVG is 512x512 with circles centered at 256,256.
    // We scale everything relative to our target radius.
    let s = scale / 256.0; // scale factor from SVG coords to our size

    struct Ring {
        radius: f32,
        stroke_width: f32,
        opacity: f32,
    }

    let rings = [
        Ring { radius: 248.0, stroke_width: 12.8, opacity: 0.25 },
        Ring { radius: 208.0, stroke_width: 12.8, opacity: 0.35 },
        Ring { radius: 168.0, stroke_width: 14.4, opacity: 0.50 },
        Ring { radius: 128.0, stroke_width: 16.0, opacity: 0.65 },
        Ring { radius: 88.0,  stroke_width: 16.0, opacity: 0.80 },
        Ring { radius: 51.2,  stroke_width: 17.6, opacity: 0.95 },
    ];

    let pw = pixmap.width() as usize;
    let ph = pixmap.height() as usize;
    let data = pixmap.data_mut();

    // Draw each ring as an anti-aliased circle stroke
    for ring in &rings {
        let r = ring.radius * s;
        let sw = ring.stroke_width * s;
        let inner = r - sw / 2.0;
        let outer = r + sw / 2.0;
        let alpha = ring.opacity;

        let x_min = ((cx - outer - 1.0) as usize).max(0);
        let x_max = ((cx + outer + 2.0) as usize).min(pw);
        let y_min = ((cy - outer - 1.0) as usize).max(0);
        let y_max = ((cy + outer + 2.0) as usize).min(ph);

        for y in y_min..y_max {
            for x in x_min..x_max {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();

                // Anti-aliased ring: full opacity between inner and outer,
                // fading at the edges
                let coverage = if dist < inner - 0.5 || dist > outer + 0.5 {
                    0.0
                } else if dist < inner + 0.5 {
                    dist - (inner - 0.5) // fade in at inner edge
                } else if dist > outer - 0.5 {
                    (outer + 0.5) - dist // fade out at outer edge
                } else {
                    1.0
                };

                let a = alpha * coverage;
                if a < 0.001 { continue; }

                let idx = (y * pw + x) * 4;
                let dst_r = data[idx] as f32 / 255.0;
                let dst_g = data[idx + 1] as f32 / 255.0;
                let dst_b = data[idx + 2] as f32 / 255.0;

                // White circle composited (must set alpha for compositor)
                data[idx]     = ((dst_r * (1.0 - a) + a) * 255.0) as u8;
                data[idx + 1] = ((dst_g * (1.0 - a) + a) * 255.0) as u8;
                data[idx + 2] = ((dst_b * (1.0 - a) + a) * 255.0) as u8;
                let dst_a = data[idx + 3] as f32 / 255.0;
                data[idx + 3] = ((dst_a + a * (1.0 - dst_a)).min(1.0) * 255.0) as u8;
            }
        }
    }

    // Center filled dot
    let dot_r = 22.4 * s;
    let x_min = ((cx - dot_r - 1.0) as usize).max(0);
    let x_max = ((cx + dot_r + 2.0) as usize).min(pw);
    let y_min = ((cy - dot_r - 1.0) as usize).max(0);
    let y_max = ((cy + dot_r + 2.0) as usize).min(ph);

    for y in y_min..y_max {
        for x in x_min..x_max {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            let a = if dist > dot_r + 0.5 {
                0.0
            } else if dist > dot_r - 0.5 {
                (dot_r + 0.5) - dist
            } else {
                1.0
            };

            if a < 0.001 { continue; }

            let idx = (y * pw + x) * 4;
            let dst_r = data[idx] as f32 / 255.0;
            let dst_g = data[idx + 1] as f32 / 255.0;
            let dst_b = data[idx + 2] as f32 / 255.0;

            data[idx]     = ((dst_r * (1.0 - a) + a) * 255.0) as u8;
            data[idx + 1] = ((dst_g * (1.0 - a) + a) * 255.0) as u8;
            data[idx + 2] = ((dst_b * (1.0 - a) + a) * 255.0) as u8;
            let dst_a = data[idx + 3] as f32 / 255.0;
            data[idx + 3] = ((dst_a + a * (1.0 - dst_a)).min(1.0) * 255.0) as u8;
        }
    }
}

/// Render paragraph text + reference ID.
/// Text block is vertically centered on screen. If text would overflow,
/// font size is progressively reduced until it fits (safety net — the text
/// chunker should prevent this in actual videos).
pub fn render_paragraph(
    renderer: &mut TextRenderer,
    pixmap: &mut Pixmap,
    text: &str,
    reference_id: &str,
) {
    let w = WIDTH as f32;
    let h = HEIGHT as f32;
    let text_block_width = 1100.0;
    let x = (w - text_block_width) / 2.0;
    let padding = 80.0; // top + bottom padding
    let max_text_height = h - padding * 2.0 - 40.0; // leave room for ref ID

    // Find a font size that fits (default body is 30px)
    let mut font_size = 30.0_f32;
    let mut text_height;
    loop {
        let measure_style = TextStyle::body_sized(0.0, 0.0, font_size);
        text_height = renderer.measure_text(text, &measure_style);
        if text_height <= max_text_height || font_size <= 18.0 {
            break;
        }
        font_size -= 2.0;
    }

    // Center vertically
    let y = (h - text_height) / 2.0;

    let body_style = TextStyle::body_sized(x, y, font_size);
    let rendered_height = renderer.render_text(pixmap, text, &body_style);

    // Reference ID — right-aligned to the text block edge
    let ref_measure = TextStyle::reference_id(0.0, 0.0);
    let ref_width = renderer.measure_text_width(reference_id, &ref_measure);
    let ref_x = x + text_block_width - ref_width;

    let ref_style = TextStyle::reference_id(ref_x, y + rendered_height + 12.0);
    renderer.render_text(pixmap, reference_id, &ref_style);
}
