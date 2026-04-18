use cosmic_text::{
    Align, Attrs, Buffer, Color as CColor, Family, FontSystem, Metrics, Shaping, SwashCache,
    Weight,
};
use tiny_skia::Pixmap;
use crate::config::*;

/// Shared font system — load once, reuse across all renders
pub struct TextRenderer {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
}

impl TextRenderer {
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();

        // Load bundled fonts
        let fonts_dir = std::path::Path::new("assets/fonts");
        for entry in std::fs::read_dir(fonts_dir).expect("assets/fonts/ not found") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "ttf") {
                let data = std::fs::read(&path).unwrap();
                font_system.db_mut().load_font_data(data);
            }
        }

        Self {
            font_system,
            swash_cache: SwashCache::new(),
        }
    }

    /// Measure text height without rendering.
    pub fn measure_text(&mut self, text: &str, style: &TextStyle) -> f32 {
        let metrics = Metrics::new(style.font_size, style.line_height);
        let attrs = Attrs::new()
            .family(style.font_family)
            .weight(style.weight);

        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(style.max_width), None);
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        buffer.layout_runs().count() as f32 * style.line_height
    }

    /// Measure the width of a single-line text string.
    pub fn measure_text_width(&mut self, text: &str, style: &TextStyle) -> f32 {
        let metrics = Metrics::new(style.font_size, style.line_height);
        let attrs = Attrs::new()
            .family(style.font_family)
            .weight(style.weight);

        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(f32::MAX), None);
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        buffer
            .layout_runs()
            .map(|run| run.line_w)
            .next()
            .unwrap_or(0.0)
    }

    /// Render text onto a pixmap. Position is the top-left of the text bounding box.
    /// Returns the height of the rendered text block.
    pub fn render_text(
        &mut self,
        pixmap: &mut Pixmap,
        text: &str,
        style: &TextStyle,
    ) -> f32 {
        let metrics = Metrics::new(style.font_size, style.line_height);

        let attrs = Attrs::new()
            .family(style.font_family)
            .weight(style.weight);

        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(style.max_width), None);
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);

        // Set alignment on all lines
        for line in buffer.lines.iter_mut() {
            line.set_align(Some(if style.center {
                Align::Center
            } else {
                Align::Left
            }));
        }

        buffer.shape_until_scroll(&mut self.font_system, false);

        let color = CColor::rgba(style.color[0], style.color[1], style.color[2], style.color[3]);

        let pw = pixmap.width() as i32;
        let ph = pixmap.height() as i32;
        let data = pixmap.data_mut();

        // Calculate text block height
        let total_height = buffer.layout_runs().count() as f32 * style.line_height;

        let base_x = style.x as i32;
        let base_y = style.y as i32;

        buffer.draw(&mut self.font_system, &mut self.swash_cache, color, |x, y, _w, _h, col| {
            let px = base_x + x;
            let py = base_y + y;

            if px < 0 || py < 0 || px >= pw || py >= ph {
                return;
            }

            let alpha = col.a() as f32 / 255.0;
            if alpha < 0.01 {
                return;
            }

            let idx = (py as usize * pw as usize + px as usize) * 4;
            if idx + 3 >= data.len() {
                return;
            }

            let src_r = col.r() as f32 / 255.0;
            let src_g = col.g() as f32 / 255.0;
            let src_b = col.b() as f32 / 255.0;

            let dst_r = data[idx] as f32 / 255.0;
            let dst_g = data[idx + 1] as f32 / 255.0;
            let dst_b = data[idx + 2] as f32 / 255.0;

            data[idx] = ((dst_r * (1.0 - alpha) + src_r * alpha) * 255.0) as u8;
            data[idx + 1] = ((dst_g * (1.0 - alpha) + src_g * alpha) * 255.0) as u8;
            data[idx + 2] = ((dst_b * (1.0 - alpha) + src_b * alpha) * 255.0) as u8;
            data[idx + 3] = 255;
        });

        total_height
    }
}

pub struct TextStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub max_width: f32,
    pub x: f32,
    pub y: f32,
    pub color: [u8; 4],
    pub font_family: Family<'static>,
    pub weight: Weight,
    pub center: bool,
}

impl TextStyle {
    // --- Paragraph body: Lora Regular, caller-supplied font_size, line-height 1.5 ---

    pub fn body(x: f32, y: f32) -> Self {
        Self::body_sized(x, y, 30.0)
    }

    pub fn body_sized(x: f32, y: f32, font_size: f32) -> Self {
        Self {
            font_size,
            line_height: font_size * 1.5,
            max_width: 1100.0 * RESOLUTION_SCALE,
            x,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lora"),
            weight: Weight::NORMAL,
            center: false,
        }
    }

    // --- Reference ID: DM Sans Regular, muted, base 26pt (scaled) ---

    pub fn reference_id(x: f32, y: f32) -> Self {
        let font_size = 26.0 * RESOLUTION_SCALE;
        Self {
            font_size,
            line_height: font_size * 1.4,
            max_width: 200.0 * RESOLUTION_SCALE,
            x,
            y,
            color: TEXT_MUTED,
            font_family: Family::Name("DM Sans"),
            weight: Weight::NORMAL,
            center: false,
        }
    }

    // --- Paper title (intro card): Lora SemiBold, base 120pt (scaled), centered ---

    pub fn paper_title(y: f32) -> Self {
        let font_size = 120.0 * RESOLUTION_SCALE;
        let max_width = 1400.0 * RESOLUTION_SCALE;
        Self {
            font_size,
            line_height: font_size * 1.25,
            max_width,
            x: (WIDTH as f32 - max_width) / 2.0,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lora"),
            weight: Weight::SEMIBOLD,
            center: true,
        }
    }

    // --- Paper label ("PAPER 1"): DM Sans, muted, base 48pt (scaled), centered ---

    pub fn paper_label(y: f32) -> Self {
        let font_size = 48.0 * RESOLUTION_SCALE;
        let max_width = 500.0 * RESOLUTION_SCALE;
        Self {
            font_size,
            line_height: font_size * 1.4,
            max_width,
            x: (WIDTH as f32 - max_width) / 2.0,
            y,
            color: TEXT_MUTED,
            font_family: Family::Name("DM Sans"),
            weight: Weight::NORMAL,
            center: true,
        }
    }

    // --- Section title: Lora Medium Italic, base 100pt (scaled), centered ---
    // Note: cosmic-text doesn't have a separate italic toggle via TextStyle,
    // so we use the italic font family name directly.

    pub fn section_title(y: f32) -> Self {
        let font_size = 100.0 * RESOLUTION_SCALE;
        let max_width = 1400.0 * RESOLUTION_SCALE;
        Self {
            font_size,
            line_height: font_size * 1.4,
            max_width,
            x: (WIDTH as f32 - max_width) / 2.0,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lora"),
            weight: Weight::MEDIUM,
            center: true,
        }
    }

    // --- Outro logo: "Urantia" in Lato Light ---

    pub fn outro_logo_light(x: f32, y: f32) -> Self {
        let font_size = 110.0 * RESOLUTION_SCALE;
        Self {
            font_size,
            line_height: font_size * 1.3,
            max_width: 800.0 * RESOLUTION_SCALE,
            x,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lato"),
            weight: Weight::LIGHT,
            center: false,
        }
    }

    // --- Outro logo: "Hub" in Lato Bold ---

    pub fn outro_logo_bold(x: f32, y: f32) -> Self {
        let font_size = 110.0 * RESOLUTION_SCALE;
        Self {
            font_size,
            line_height: font_size * 1.3,
            max_width: 500.0 * RESOLUTION_SCALE,
            x,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lato"),
            weight: Weight::BOLD,
            center: false,
        }
    }

    // --- Outro subtitle: DM Sans, muted, centered ---

    pub fn outro_subtitle(y: f32) -> Self {
        let font_size = 36.0 * RESOLUTION_SCALE;
        let max_width = 900.0 * RESOLUTION_SCALE;
        Self {
            font_size,
            line_height: font_size * 1.4,
            max_width,
            x: (WIDTH as f32 - max_width) / 2.0,
            y,
            color: TEXT_MUTED,
            font_family: Family::Name("DM Sans"),
            weight: Weight::NORMAL,
            center: true,
        }
    }

    // --- Thumbnail styles (2.5x larger for YouTube thumbnails) ---

    pub fn thumbnail_label(y: f32) -> Self {
        let max_width = 1200.0;
        Self {
            font_size: 55.0,
            line_height: 55.0 * 1.4,
            max_width,
            x: (WIDTH as f32 - max_width) / 2.0,
            y,
            color: TEXT_MUTED,
            font_family: Family::Name("DM Sans"),
            weight: Weight::NORMAL,
            center: true,
        }
    }

    pub fn thumbnail_title(y: f32) -> Self {
        let max_width = 1400.0;
        Self {
            font_size: 120.0,
            line_height: 120.0 * 1.2,
            max_width,
            x: (WIDTH as f32 - max_width) / 2.0,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lora"),
            weight: Weight::SEMIBOLD,
            center: true,
        }
    }

    // --- Paper thumbnail (redesigned): "PAPER 1" / "FOREWORD" in DM Sans Bold,
    //     left-aligned within the right-hand text column ---

    pub fn thumbnail_paper_number(x: f32, y: f32, max_width: f32, scale: f32) -> Self {
        let font_size = 160.0 * scale;
        Self {
            font_size,
            line_height: font_size * 1.1,
            max_width,
            x,
            y,
            // Classic gold (#D4A84A) — warmer and more restrained than a yellow-gold
            color: [212, 168, 74, 255],
            font_family: Family::Name("Lato"),
            weight: Weight::BOLD,
            center: false,
        }
    }

    // --- Paper thumbnail (redesigned): paper title in Lora SemiBold, left-aligned ---

    pub fn thumbnail_paper_title_right(x: f32, y: f32, max_width: f32, scale: f32) -> Self {
        let font_size = 130.0 * scale;
        Self {
            font_size,
            line_height: font_size * 1.15,
            max_width,
            x,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lora"),
            weight: Weight::SEMIBOLD,
            center: false,
        }
    }

    // --- Channel banner: "UrantiaHub" wordmark (Lato Light + Bold), large ---

    pub fn banner_wordmark_light(x: f32, y: f32) -> Self {
        Self {
            font_size: 170.0,
            line_height: 170.0 * 1.1,
            max_width: 900.0,
            x,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lato"),
            weight: Weight::LIGHT,
            center: false,
        }
    }

    pub fn banner_wordmark_bold(x: f32, y: f32) -> Self {
        Self {
            font_size: 170.0,
            line_height: 170.0 * 1.1,
            max_width: 500.0,
            x,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lato"),
            weight: Weight::BOLD,
            center: false,
        }
    }

    // --- Channel banner: tagline (Lora Regular, below wordmark) ---

    pub fn banner_tagline(x: f32, y: f32) -> Self {
        Self {
            font_size: 44.0,
            line_height: 44.0 * 1.3,
            max_width: 1000.0,
            x,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lora"),
            weight: Weight::NORMAL,
            center: false,
        }
    }

    // --- Channel banner: URL footer (DM Sans Medium, gold) ---

    pub fn banner_url(x: f32, y: f32) -> Self {
        Self {
            font_size: 34.0,
            line_height: 34.0 * 1.3,
            max_width: 600.0,
            x,
            y,
            color: [212, 168, 74, 255],
            font_family: Family::Name("DM Sans"),
            weight: Weight::MEDIUM,
            center: false,
        }
    }
}
