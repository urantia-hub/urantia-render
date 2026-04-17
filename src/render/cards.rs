use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};
use crate::config::*;
use crate::render::text::{TextRenderer, TextStyle};

/// Render a playlist thumbnail — label (e.g. "Part I") above + title below, centered.
/// If label is empty, renders title + subtitle layout instead.
pub fn render_playlist_thumbnail(
    renderer: &mut TextRenderer,
    pixmap: &mut Pixmap,
    label: &str,
    title: &str,
) {
    render_playlist_thumbnail_with_subtitle(renderer, pixmap, label, title, None);
}

/// Render a playlist thumbnail with optional subtitle below the title.
pub fn render_playlist_thumbnail_with_subtitle(
    renderer: &mut TextRenderer,
    pixmap: &mut Pixmap,
    label: &str,
    title: &str,
    subtitle: Option<&str>,
) {
    // Layout designed at 1920×1080 reference; scale with pixmap dimensions.
    let scale = pixmap.width() as f32 / 1920.0;
    let h = pixmap.height() as f32;

    let logo_cx = 380.0 * scale;
    let logo_cy = h / 2.0;
    let logo_radius = 290.0 * scale;
    render_concentric_logo(pixmap, logo_cx, logo_cy, logo_radius);

    let text_x = 720.0 * scale;
    let text_max_width = 1160.0 * scale;
    let gap = 40.0 * scale;

    let label_or_master = if label.is_empty() {
        "URANTIA PAPERS".to_string()
    } else {
        label.to_uppercase()
    };

    let label_measure = TextStyle::thumbnail_paper_number(text_x, 0.0, text_max_width, scale);
    let label_height = renderer.measure_text(&label_or_master, &label_measure);

    let title_measure = TextStyle::thumbnail_paper_title_right(text_x, 0.0, text_max_width, scale);
    let title_height = renderer.measure_text(title, &title_measure);

    let subtitle_height = subtitle
        .map(|s| renderer.measure_text(s, &title_measure))
        .unwrap_or(0.0);
    let subtitle_gap = if subtitle.is_some() { gap } else { 0.0 };

    let total_height = label_height + gap + title_height + subtitle_gap + subtitle_height;
    let start_y = (h - total_height) / 2.0;

    let label_style = TextStyle::thumbnail_paper_number(text_x, start_y, text_max_width, scale);
    renderer.render_text(pixmap, &label_or_master, &label_style);

    let title_y = start_y + label_height + gap;
    let title_style = TextStyle::thumbnail_paper_title_right(text_x, title_y, text_max_width, scale);
    renderer.render_text(pixmap, title, &title_style);

    if let Some(sub) = subtitle {
        let sub_y = title_y + title_height + subtitle_gap;
        let sub_style = TextStyle::thumbnail_paper_title_right(text_x, sub_y, text_max_width, scale);
        renderer.render_text(pixmap, sub, &sub_style);
    }
}

/// Render a YouTube thumbnail: UrantiaHub concentric-rings logo on the left,
/// big "PAPER N" label and paper title stacked on the right. Designed for
/// browse-view legibility at mobile sizes.
pub fn render_thumbnail(
    renderer: &mut TextRenderer,
    pixmap: &mut Pixmap,
    paper_id: &str,
    paper_title: &str,
) {
    // Layout is designed at 1920×1080; scale up for 4K thumbnails.
    let scale = pixmap.width() as f32 / 1920.0;
    let h = pixmap.height() as f32;

    let logo_cx = 380.0 * scale;
    let logo_cy = h / 2.0;
    let logo_radius = 290.0 * scale;
    render_concentric_logo(pixmap, logo_cx, logo_cy, logo_radius);

    let text_x = 720.0 * scale;
    let text_max_width = 1160.0 * scale;
    let gap = 40.0 * scale;

    let label = if paper_id == "0" {
        "FOREWORD".to_string()
    } else {
        format!("PAPER {}", paper_id)
    };

    let number_measure = TextStyle::thumbnail_paper_number(text_x, 0.0, text_max_width, scale);
    let number_height = renderer.measure_text(&label, &number_measure);

    let title_height = if paper_id == "0" {
        0.0
    } else {
        let title_measure =
            TextStyle::thumbnail_paper_title_right(text_x, 0.0, text_max_width, scale);
        renderer.measure_text(paper_title, &title_measure)
    };

    let effective_gap = if paper_id == "0" { 0.0 } else { gap };
    let total_height = number_height + effective_gap + title_height;
    let start_y = (h - total_height) / 2.0;

    let number_style = TextStyle::thumbnail_paper_number(text_x, start_y, text_max_width, scale);
    renderer.render_text(pixmap, &label, &number_style);

    if paper_id != "0" {
        let title_y = start_y + number_height + gap;
        let title_style =
            TextStyle::thumbnail_paper_title_right(text_x, title_y, text_max_width, scale);
        renderer.render_text(pixmap, paper_title, &title_style);
    }
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

    let label_style = TextStyle::paper_label(center_y - 48.0 * RESOLUTION_SCALE);
    renderer.render_text(pixmap, &label, &label_style);

    let title_style = TextStyle::paper_title(center_y - 12.0 * RESOLUTION_SCALE);
    renderer.render_text(pixmap, paper_title, &title_style);
}

/// Render section card — section title centered on screen
pub fn render_section_card(
    renderer: &mut TextRenderer,
    pixmap: &mut Pixmap,
    section_title: &str,
) {
    let h = HEIGHT as f32;
    let style = TextStyle::section_title(h / 2.0 - 30.0 * RESOLUTION_SCALE);
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
    let logo_radius = 56.0 * RESOLUTION_SCALE;
    let logo_cx = w / 2.0;
    let logo_cy = center_y - 65.0 * RESOLUTION_SCALE;
    render_concentric_logo(pixmap, logo_cx, logo_cy, logo_radius);

    // "Urantia" (Lato Light) + "Hub" (Lato Bold) side by side
    let text_y = logo_cy + logo_radius + 15.0 * RESOLUTION_SCALE;

    let urantia_width = renderer.measure_text_width("Urantia", &TextStyle::outro_logo_light(0.0, 0.0));
    let hub_width = renderer.measure_text_width("Hub", &TextStyle::outro_logo_bold(0.0, 0.0));
    let total_width = urantia_width + hub_width;
    let text_x = (w - total_width) / 2.0;

    let light_style = TextStyle::outro_logo_light(text_x, text_y);
    renderer.render_text(pixmap, "Urantia", &light_style);

    // Light weight sits higher — nudge "Hub" up to align baselines
    let bold_style = TextStyle::outro_logo_bold(text_x + urantia_width, text_y - 5.0 * RESOLUTION_SCALE);
    renderer.render_text(pixmap, "Hub", &bold_style);

    // Subtitle below
    let subtitle_text = tagline.unwrap_or("urantiahub.com");
    let subtitle_style = TextStyle::outro_subtitle(text_y + 62.0 * RESOLUTION_SCALE);
    renderer.render_text(pixmap, subtitle_text, &subtitle_style);
}

/// Render the UrantiaHub concentric-rings logo onto `pixmap`.
/// `cx, cy` is the logo center. `scale` is the outer radius in pixels
/// (pass 300 for a ~600 px diameter logo; 400 for ~800 px diameter).
/// 7 circles: 6 stroked rings (increasing opacity) + 1 filled center dot.
///
/// Uses tiny-skia's anti-aliased path stroker, which produces clean edges
/// against any background (flat or gradient).
pub fn render_concentric_logo(pixmap: &mut Pixmap, cx: f32, cy: f32, scale: f32) {
    use tiny_skia::{LineCap, Stroke};

    // Original SVG is 512x512 with circles centered at 256,256.
    let s = scale / 256.0;

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

    for ring in &rings {
        let r = ring.radius * s;
        let path = PathBuilder::from_circle(cx, cy, r)
            .expect("failed to build ring path");

        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba(1.0, 1.0, 1.0, ring.opacity).unwrap());
        paint.anti_alias = true;

        let stroke = Stroke {
            width: ring.stroke_width * s,
            line_cap: LineCap::Butt,
            ..Stroke::default()
        };

        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    // Center filled dot
    let dot_r = 22.4 * s;
    let dot_path = PathBuilder::from_circle(cx, cy, dot_r)
        .expect("failed to build center dot path");

    let mut dot_paint = Paint::default();
    dot_paint.set_color(Color::WHITE);
    dot_paint.anti_alias = true;

    pixmap.fill_path(&dot_path, &dot_paint, FillRule::Winding, Transform::identity(), None);
}

/// Render the YouTube channel banner at 2560×1440 (passed in via `pixmap`).
///
/// Layout centered in the 1546×423 safe area:
///   [Logo 420 diameter]  |  UrantiaHub (220 pt wordmark)
///                        |  All 197 Urantia Papers. Audio and text.
///                        |  urantiahub.com
pub fn render_banner(renderer: &mut TextRenderer, pixmap: &mut Pixmap) {
    let w = pixmap.width() as f32;
    let h = pixmap.height() as f32;

    // YouTube "all devices" safe area — content here is visible on mobile too.
    // 1235×338 centered, per YouTube channel banner spec.
    let safe_w = 1235.0;
    let safe_h = 338.0;
    let safe_x = (w - safe_w) / 2.0;
    let safe_y = (h - safe_h) / 2.0;

    let logo_cx = safe_x + 155.0;
    let logo_cy = safe_y + safe_h / 2.0;
    let logo_radius = 155.0;
    render_concentric_logo(pixmap, logo_cx, logo_cy, logo_radius);

    let text_x = safe_x + 360.0;

    let light_measure = TextStyle::banner_wordmark_light(0.0, 0.0);
    let urantia_w = renderer.measure_text_width("Urantia", &light_measure);
    let wordmark_h = 170.0 * 1.1;

    let tagline = "All 197 Urantia Papers. Audio and text.";
    let tagline_measure = TextStyle::banner_tagline(text_x, 0.0);
    let tagline_h = renderer.measure_text(tagline, &tagline_measure);

    let url = "urantiahub.com";
    let url_h = 34.0 * 1.3;

    let gap1 = 14.0;
    let gap2 = 18.0;
    let stack_h = wordmark_h + gap1 + tagline_h + gap2 + url_h;
    let stack_top = safe_y + (safe_h - stack_h) / 2.0;

    let wordmark_x = text_x;
    let light = TextStyle::banner_wordmark_light(wordmark_x, stack_top);
    renderer.render_text(pixmap, "Urantia", &light);
    // Lato Light renders ~12 px higher than Lato Bold at 170pt in cosmic-text;
    // nudge "Hub" up to put both on the same visual baseline.
    let bold = TextStyle::banner_wordmark_bold(wordmark_x + urantia_w, stack_top - 18.0);
    renderer.render_text(pixmap, "Hub", &bold);

    let tagline_y = stack_top + wordmark_h + gap1;
    let tag = TextStyle::banner_tagline(text_x, tagline_y);
    renderer.render_text(pixmap, tagline, &tag);

    let url_y = tagline_y + tagline_h + gap2;
    let url_style = TextStyle::banner_url(text_x, url_y);
    renderer.render_text(pixmap, url, &url_style);
}

/// Render a YouTube channel profile icon: logo centered on a dark solid
/// background. Sized by pixmap dimensions (recommended 1024×1024).
/// YouTube auto-crops to a circle, so all content stays within the inscribed
/// circle (diameter ≈ pixmap min dimension).
pub fn render_channel_icon(pixmap: &mut Pixmap) {
    let w = pixmap.width() as f32;
    let h = pixmap.height() as f32;
    let cx = w / 2.0;
    let cy = h / 2.0;

    let logo_radius = (w.min(h) / 2.0) * 0.82;
    render_concentric_logo(pixmap, cx, cy, logo_radius);
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
    let text_block_width = 1100.0 * RESOLUTION_SCALE;
    let x = (w - text_block_width) / 2.0;
    let padding = 80.0 * RESOLUTION_SCALE;
    let max_text_height = h - padding * 2.0 - 40.0 * RESOLUTION_SCALE; // leave room for ref ID

    // Body starts at 48pt (up from the old 30pt default) and scales with RESOLUTION_SCALE.
    let min_font_size = 28.0 * RESOLUTION_SCALE;
    let shrink_step = 2.0 * RESOLUTION_SCALE;
    let mut font_size = 48.0 * RESOLUTION_SCALE;
    let mut text_height;
    loop {
        let measure_style = TextStyle::body_sized(0.0, 0.0, font_size);
        text_height = renderer.measure_text(text, &measure_style);
        if text_height <= max_text_height || font_size <= min_font_size {
            break;
        }
        font_size -= shrink_step;
    }

    // Center vertically
    let y = (h - text_height) / 2.0;

    let body_style = TextStyle::body_sized(x, y, font_size);
    let rendered_height = renderer.render_text(pixmap, text, &body_style);

    // Reference ID — right-aligned to the text block edge
    let ref_measure = TextStyle::reference_id(0.0, 0.0);
    let ref_width = renderer.measure_text_width(reference_id, &ref_measure);
    let ref_x = x + text_block_width - ref_width;

    let ref_style = TextStyle::reference_id(ref_x, y + rendered_height + 12.0 * RESOLUTION_SCALE);
    renderer.render_text(pixmap, reference_id, &ref_style);
}
