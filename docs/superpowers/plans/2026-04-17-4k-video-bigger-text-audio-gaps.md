# 4K Video, Bigger Text, and Audio Breathing Room — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-render the 197 Urantia Paper videos at 4K (3840×2160) with larger text, brighter muted reference IDs, and audible silence gaps between paragraphs / around section cards / after the paper title intro — so YouTube's re-encoder has more bitrate budget per pixel (fixing "pixelated" look) and the listener gets breathing room between spoken segments.

**Architecture:** Introduce a single `RESOLUTION_SCALE` multiplier (= 2 at 4K) derived from `WIDTH`/`HEIGHT` in `config.rs`, and multiply every hardcoded pixel dimension in the render pipeline by it (font sizes, orb radii, text block widths, card margins). Leave the `thumbnail` and `banner` subcommands at their current fixed output sizes by making them build their own pixmaps explicitly rather than inheriting `config::WIDTH/HEIGHT`. Add four new timing constants to `config.rs` (`GAP_AFTER_INTRO_SEC`, `GAP_BEFORE_SECTION_SEC`, `GAP_AFTER_SECTION_SEC`, `GAP_BETWEEN_PARAGRAPHS_SEC`) and bump `current_frame` in `build_manifest` between segments accordingly — the zero-initialized PCM buffer in `audio/concat.rs` naturally fills the new gaps with silence.

**Tech Stack:** Rust 1.86, tiny-skia 0.11, cosmic-text 0.12, symphonia (PCM decode), ffmpeg subprocess. No new dependencies.

---

## Context

The 120 uploaded videos use these defaults: 1920×1080 canvas, paragraph body 30pt, ref ID 16pt in near-invisible gray (`#636263`), back-to-back audio segments with only visual fades. On YouTube the result is pixelated gray text on dark — YouTube's 1080p VP9 ceiling (~8 Mbps) is brutal to thin gray anti-aliased edges.

Data from the audit:

1. Only `src/config.rs` lines 2-3 need a resolution swap; almost all layout math is already `WIDTH`-relative.
2. `src/render/background.rs:22-47` has absolute pixel radii (500, 400, 450, 420, 350 px) for the three orbs — these MUST scale with canvas or the background will look tiny at 4K.
3. `src/render/cards.rs:380` hardcodes a `text_block_width = 1100.0` for paragraph body layout — must scale.
4. Most `TextStyle` presets use fixed `max_width` values (900, 800, 1100, etc.) that look correct relative to 1920 — must scale with resolution.
5. `cmd_thumbnails` in `src/main.rs:694` builds its canvas as `Pixmap::new(config::WIDTH, config::HEIGHT)` — will incorrectly scale to 4K unless decoupled.
6. `build_manifest` in `src/data/manifest.rs:89, 111, 137` goes directly from one segment's end to the next — no silence between paragraphs.
7. Intro padding (`INTRO_PADDING_SEC = 1.0`) and section padding (`SECTION_CARD_PADDING_SEC = 1.0`) ARE additive inside those segments' durations — the audio finishes early and the visual holds in silence before the next segment begins. This existing silence counts toward the "gap after intro" / "gap after section" behavior Kelson asked for, but is currently only 1 second. The plan adds explicit additional gaps so the total feels like the desired pause length.
8. `audio/concat.rs:121` pre-fills the output buffer with zeros and writes MP3 samples at frame-computed offsets — shifting `start_frame` automatically produces silence in the gaps.

Chosen constants (user directive: "longer", "even bigger between para→section→para", "bigger than normal between title+1st paragraph"):

```rust
// Existing (kept, these are part of the segment duration, not between segments):
INTRO_PADDING_SEC         = 1.0  // silence held at end of intro card
SECTION_CARD_PADDING_SEC  = 1.0  // silence held at end of section card

// New (inserted between segments as extra silence):
GAP_AFTER_INTRO_SEC          = 1.5  // intro → first paragraph
GAP_BETWEEN_PARAGRAPHS_SEC   = 0.6  // paragraph → paragraph within a section
GAP_BEFORE_SECTION_SEC       = 1.2  // last paragraph → section card
GAP_AFTER_SECTION_SEC        = 1.0  // section card → first paragraph
```

Combined effect: intro→paragraph ≈ 1.0 + 1.5 = 2.5 s of silence. paragraph→section→paragraph ≈ 0.6 * 0 + 1.2 + (section card duration) + 1.0 + 1.0 = plenty. paragraph→paragraph = 0.6 s.

Fade logic is frame-count based (15-frame fade-in / fade-out applied inside each segment's duration) and is not affected by gap changes — we simply extend `start_frame` of the next segment. During the gap the previous segment has already faded to 0 opacity (background only on screen), then the next segment fades in — exactly the desired behavior.

---

## File Structure

- **Modify:** `urantia-render/src/config.rs` — bump WIDTH/HEIGHT to 4K; add RESOLUTION_SCALE; add four gap constants; brighten TEXT_MUTED.
- **Modify:** `urantia-render/src/render/background.rs` — scale orb radii by RESOLUTION_SCALE.
- **Modify:** `urantia-render/src/render/text.rs` — scale every font_size / line_height / max_width by RESOLUTION_SCALE; also bump base sizes (body 30→48, ref 16→26, paper_label 22→32, paper_title 54→72).
- **Modify:** `urantia-render/src/render/cards.rs` — scale paragraph text_block_width + padding; scale outro logo logo_radius and text offsets; do NOT modify thumbnail or banner functions (they stay at 1920×1080 / 2560×1440 respectively).
- **Modify:** `urantia-render/src/main.rs` — decouple `cmd_thumbnails` from `config::WIDTH/HEIGHT`, hardcode 1920×1080 for thumbnails.
- **Modify:** `urantia-render/src/data/manifest.rs` — add gap insertion between segments per the constants in config.rs; re-compute total_duration_frames accordingly.
- **No changes needed:** `urantia-render/src/audio/concat.rs` (zero-filled buffer naturally fills new silence), `urantia-render/src/encode/ffmpeg.rs` (already uses config globals), `urantia-render/src/render/frame.rs` (already uses WIDTH/HEIGHT).

Files that are NOT changed: `render_concentric_logo` (already radius-parameterized), `render_thumbnail` (stays at 1920×1080), `render_banner` (stays at 2560×1440), `render_channel_icon` (size passed in via param).

---

## Tasks

### Task 1: Bump TEXT_MUTED to a codec-safe brightness

**Files:**
- Modify: `urantia-render/src/config.rs:18-21`

- [ ] **Step 1: Read current color**

Open `urantia-render/src/config.rs`. Confirm line 21 reads:
```rust
pub const TEXT_MUTED: [u8; 4] = [99, 98, 99, 255];
```
This is `#636263` — too close to BG_COLOR `#0a0a0f` in luminance; YouTube VP9 re-encode mangles it.

- [ ] **Step 2: Replace with brighter muted color**

Replace lines 19-21 with:

```rust
// Muted text: rgba(232,230,225,0.6) composited on #0a0a0f → solid RGB
// R: 10 * 0.4 + 232 * 0.6 = 143, G: 10 * 0.4 + 230 * 0.6 = 142, B: 15 * 0.4 + 225 * 0.4 = 141
// (At 0.4 alpha #636263 gets smashed by YouTube re-encoding; 0.6 stays readable.)
pub const TEXT_MUTED: [u8; 4] = [143, 142, 141, 255];
```

- [ ] **Step 3: Build to confirm nothing else references the old value**

Run: `cd urantia-render && cargo build 2>&1 | tail -5`
Expected: `Finished ... target(s)` with no new errors.

- [ ] **Step 4: Commit**

```bash
cd urantia-render
git add src/config.rs
git commit -m "brighten TEXT_MUTED so YouTube reencoding preserves reference IDs"
```

---

### Task 2: Add RESOLUTION_SCALE and bump to 4K

**Files:**
- Modify: `urantia-render/src/config.rs:1-4`

- [ ] **Step 1: Replace the video block at the top of config.rs**

In `urantia-render/src/config.rs`, replace lines 1-4 (the `// Video` block):

From:
```rust
// Video
pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;
pub const FPS: u32 = 30;
```

To:
```rust
// Video — render at 4K so YouTube's per-resolution bitrate tier has enough
// budget to keep gray text on dark backgrounds from pixelating.
pub const WIDTH: u32 = 3840;
pub const HEIGHT: u32 = 2160;
pub const FPS: u32 = 30;

/// Multiplier for any pixel dimension designed at the 1920×1080 reference size.
/// Used to scale font sizes, orb radii, padding, etc.
pub const RESOLUTION_SCALE: f32 = WIDTH as f32 / 1920.0;
```

- [ ] **Step 2: Build — expect it still compiles (nothing consumes RESOLUTION_SCALE yet)**

Run: `cd urantia-render && cargo build 2>&1 | tail -5`
Expected: builds successfully with one `dead_code` warning for `RESOLUTION_SCALE`.

- [ ] **Step 3: Commit**

```bash
cd urantia-render
git add src/config.rs
git commit -m "bump render resolution to 4K and add RESOLUTION_SCALE helper"
```

---

### Task 3: Scale background orbs so they don't shrink visually at 4K

**Files:**
- Modify: `urantia-render/src/render/background.rs:18-82`

- [ ] **Step 1: Change orb-rendering to apply RESOLUTION_SCALE**

Open `urantia-render/src/render/background.rs`. Replace the `render_background` function body and the orb-rendering call so radii scale. Lines 57-82 currently read:

```rust
/// Render the animated glow background at a given time (seconds).
/// Returns a 1920x1080 RGBA pixmap.
pub fn render_background(time_sec: f64) -> Pixmap {
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).unwrap();

    // Fill with dark background
    pixmap.fill(Color::from_rgba8(BG_COLOR[0], BG_COLOR[1], BG_COLOR[2], BG_COLOR[3]));

    let t = time_sec as f32;

    for orb in &ORBS {
        let cx = (0.5 + orb.x_amp * (t * orb.x_freq + orb.x_phase).sin()) * WIDTH as f32;
        let cy = (0.5 + orb.y_amp * (t * orb.y_freq + orb.y_phase).cos()) * HEIGHT as f32;

        render_soft_ellipse(
            &mut pixmap,
            cx,
            cy,
            orb.radius_x,
            orb.radius_y,
            orb.color,
        );
    }

    pixmap
}
```

Replace with:

```rust
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
```

- [ ] **Step 2: Build**

Run: `cd urantia-render && cargo build 2>&1 | tail -5`
Expected: builds (`RESOLUTION_SCALE` is now consumed — dead_code warning gone).

- [ ] **Step 3: Smoke test — render a single frame's background**

Run: `cd urantia-render && cargo test --lib render::background`
Expected: `test_render_background ... ok` (the existing test just verifies the pixmap is created at the configured WIDTH×HEIGHT).

- [ ] **Step 4: Commit**

```bash
cd urantia-render
git add src/render/background.rs
git commit -m "scale background orbs by RESOLUTION_SCALE for 4K rendering"
```

---

### Task 4: Scale every TextStyle preset by RESOLUTION_SCALE and bump video base sizes

**Files:**
- Modify: `urantia-render/src/render/text.rs` (multiple TextStyle presets)

Font sizes designed at 1080p need to scale to stay relatively the same size at 4K. Additionally, bump the base-at-1080p sizes for the three video-content presets Kelson flagged (too small for read-along): body 30→48, paper_label 22→32, paper_title 54→72, reference_id 16→26, section_title 36→48.

All thumbnail and banner presets (thumbnail_paper_number, thumbnail_paper_title_right, thumbnail_label, thumbnail_title, banner_*) stay at their current absolute pixel sizes — they don't render on the video pipeline.

- [ ] **Step 1: Scale `reference_id` preset (lines 181-193)**

Replace the `reference_id` function with:

```rust
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
```

- [ ] **Step 2: Scale `paper_title` preset (lines 197-210)**

Replace with:

```rust
    pub fn paper_title(y: f32) -> Self {
        let font_size = 72.0 * RESOLUTION_SCALE;
        let max_width = 900.0 * RESOLUTION_SCALE;
        Self {
            font_size,
            line_height: font_size * 1.3,
            max_width,
            x: (WIDTH as f32 - max_width) / 2.0,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lora"),
            weight: Weight::SEMIBOLD,
            center: true,
        }
    }
```

- [ ] **Step 3: Scale `paper_label` preset (lines 214-227)**

Replace with:

```rust
    pub fn paper_label(y: f32) -> Self {
        let font_size = 32.0 * RESOLUTION_SCALE;
        let max_width = 400.0 * RESOLUTION_SCALE;
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
```

- [ ] **Step 4: Scale `section_title` preset (lines 233-246)**

Replace with:

```rust
    pub fn section_title(y: f32) -> Self {
        let font_size = 48.0 * RESOLUTION_SCALE;
        let max_width = 800.0 * RESOLUTION_SCALE;
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
```

- [ ] **Step 5: Scale outro presets (`outro_logo_light`, `outro_logo_bold`, `outro_subtitle`) lines 250-293**

Replace each so their font_size, line_height, and max_width multiply by `RESOLUTION_SCALE`:

```rust
    pub fn outro_logo_light(x: f32, y: f32) -> Self {
        let font_size = 48.0 * RESOLUTION_SCALE;
        Self {
            font_size,
            line_height: font_size * 1.3,
            max_width: 500.0 * RESOLUTION_SCALE,
            x,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lato"),
            weight: Weight::LIGHT,
            center: false,
        }
    }

    pub fn outro_logo_bold(x: f32, y: f32) -> Self {
        let font_size = 48.0 * RESOLUTION_SCALE;
        Self {
            font_size,
            line_height: font_size * 1.3,
            max_width: 300.0 * RESOLUTION_SCALE,
            x,
            y,
            color: TEXT_COLOR,
            font_family: Family::Name("Lato"),
            weight: Weight::BOLD,
            center: false,
        }
    }

    pub fn outro_subtitle(y: f32) -> Self {
        let font_size = 20.0 * RESOLUTION_SCALE;
        let max_width = 800.0 * RESOLUTION_SCALE;
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
```

- [ ] **Step 6: Scale the `body_sized` constructor — this is the paragraph body**

Find the `body_sized` preset (around line 397). It currently takes an explicit `font_size` argument; we only need to scale its internal `max_width` and `line_height`:

```rust
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
```

Note: `font_size` here is passed in by the caller (`render_paragraph` in cards.rs). We'll update THAT caller's default base size in Task 5.

- [ ] **Step 7: Leave thumbnail and banner presets untouched**

Scroll through the rest of the file. Any preset whose name starts with `thumbnail_` or `banner_` — leave its font_size, line_height, and max_width as-is. These feed the standalone `thumbnail` and `banner` CLI subcommands which render to fixed non-4K canvases.

- [ ] **Step 8: Make RESOLUTION_SCALE imported**

At the top of `src/render/text.rs`, confirm the import line already pulls `use crate::config::*;` (which brings in RESOLUTION_SCALE). If it reads `use crate::config::{WIDTH, TEXT_COLOR, TEXT_MUTED};` (explicit list), add `RESOLUTION_SCALE` to it.

- [ ] **Step 9: Build**

Run: `cd urantia-render && cargo build 2>&1 | tail -10`
Expected: builds cleanly.

- [ ] **Step 10: Commit**

```bash
cd urantia-render
git add src/render/text.rs
git commit -m "scale video text sizes by RESOLUTION_SCALE and bump base sizes for legibility"
```

---

### Task 5: Bump paragraph body default size to 48pt in `render_paragraph`

**Files:**
- Modify: `urantia-render/src/render/cards.rs:378-410`

Currently `render_paragraph` starts from `font_size = 30.0_f32` and shrinks to fit. At 4K the paragraph body should start from 48pt at the 1080p reference — i.e. `48 * RESOLUTION_SCALE`.

- [ ] **Step 1: Replace the paragraph body layout block**

Open `urantia-render/src/render/cards.rs`. The `render_paragraph` function is near the bottom. Replace:

```rust
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
```

with:

```rust
    let w = WIDTH as f32;
    let h = HEIGHT as f32;
    let text_block_width = 1100.0 * RESOLUTION_SCALE;
    let x = (w - text_block_width) / 2.0;
    let padding = 80.0 * RESOLUTION_SCALE;
    let max_text_height = h - padding * 2.0 - 40.0 * RESOLUTION_SCALE; // leave room for ref ID

    // Default body is 48pt (at the 1080p reference; scaled up at 4K).
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
```

Then scale the reference-ID offset a few lines below:

```rust
    let ref_style = TextStyle::reference_id(ref_x, y + rendered_height + 12.0);
```

becomes:

```rust
    let ref_style = TextStyle::reference_id(ref_x, y + rendered_height + 12.0 * RESOLUTION_SCALE);
```

- [ ] **Step 2: Ensure RESOLUTION_SCALE is in scope**

Confirm `use crate::config::*;` at the top of `src/render/cards.rs` pulls RESOLUTION_SCALE in. If an explicit import list is used, add `RESOLUTION_SCALE` to it.

- [ ] **Step 3: Scale outro logo radius + text offsets**

Still in `src/render/cards.rs`, find `render_outro_card` (around line 140-181). The logo radius, vertical text offsets, and subtitle offset are currently hardcoded in pixels. Find the lines that look like this (the exact numbers may vary slightly):

```rust
    let logo_cx = w / 2.0;
    let logo_cy = (h / 2.0) - 80.0;
    let logo_radius = 140.0;
    render_concentric_logo(pixmap, logo_cx, logo_cy, logo_radius);

    // "Urantia" (Lato Light) + "Hub" (Lato Bold) side by side
    let text_y = logo_cy + logo_radius + 15.0;
```

Scale the offsets and the logo radius:

```rust
    let logo_cx = w / 2.0;
    let logo_cy = (h / 2.0) - 80.0 * RESOLUTION_SCALE;
    let logo_radius = 140.0 * RESOLUTION_SCALE;
    render_concentric_logo(pixmap, logo_cx, logo_cy, logo_radius);

    // "Urantia" (Lato Light) + "Hub" (Lato Bold) side by side
    let text_y = logo_cy + logo_radius + 15.0 * RESOLUTION_SCALE;
```

Also scale the Hub baseline nudge (`text_y - 5.0` becomes `text_y - 5.0 * RESOLUTION_SCALE`) and the subtitle y offset (`text_y + 62.0` becomes `text_y + 62.0 * RESOLUTION_SCALE`).

- [ ] **Step 4: Scale intro card layout**

Still in `src/render/cards.rs`, find `render_intro_card` (around line 30-60). It positions `paper_label` and `paper_title` at hardcoded y values. Find the y values and multiply each by `RESOLUTION_SCALE`:

```rust
    renderer.render_text(pixmap, &format!("Paper {}", paper_id), &TextStyle::paper_label(h / 2.0 - 40.0));
    renderer.render_text(pixmap, paper_title, &TextStyle::paper_title(h / 2.0));
```

becomes:

```rust
    renderer.render_text(pixmap, &format!("Paper {}", paper_id), &TextStyle::paper_label(h / 2.0 - 40.0 * RESOLUTION_SCALE));
    renderer.render_text(pixmap, paper_title, &TextStyle::paper_title(h / 2.0));
```

If you find additional hardcoded y offsets in the intro or section card layouts (grep for decimal pixel numbers like `100.0`, `60.0`, `-40.0`), multiply each by `RESOLUTION_SCALE`.

- [ ] **Step 5: Scale section card layout**

Find `render_section_card` (around line 100-130). Do the same — any hardcoded y offset multiplied by `RESOLUTION_SCALE`.

- [ ] **Step 6: Build**

Run: `cd urantia-render && cargo build 2>&1 | tail -10`
Expected: builds cleanly.

- [ ] **Step 7: Commit**

```bash
cd urantia-render
git add src/render/cards.rs
git commit -m "scale card layouts and paragraph body for 4K rendering"
```

---

### Task 6: Keep `cmd_thumbnails` at 1920×1080 even though config is 4K

**Files:**
- Modify: `urantia-render/src/main.rs:679-710` (cmd_thumbnails body)

The thumbnail subcommand is used for YouTube thumbnails, which should stay at 1920×1080 regardless of the video canvas.

- [ ] **Step 1: Replace thumbnail canvas sizing**

In `urantia-render/src/main.rs`, in the `cmd_thumbnails` function, the canvas is currently built from `config::WIDTH, config::HEIGHT` (now 4K). Replace with explicit 1920×1080:

Find:
```rust
        let mut pixmap = render::background::render_background(2.5);
        let mut content = tiny_skia::Pixmap::new(config::WIDTH, config::HEIGHT).unwrap();
```

Replace with:
```rust
        // Thumbnails are YouTube browse-view assets — keep at 1920×1080 regardless
        // of the video canvas resolution. Build a 1920×1080 dark fill instead of
        // calling render_background (which uses config::WIDTH/HEIGHT).
        let mut pixmap = tiny_skia::Pixmap::new(1920, 1080).unwrap();
        {
            let data = pixmap.data_mut();
            for i in (0..data.len()).step_by(4) {
                data[i]     = config::BG_COLOR[0];
                data[i + 1] = config::BG_COLOR[1];
                data[i + 2] = config::BG_COLOR[2];
                data[i + 3] = config::BG_COLOR[3];
            }
        }
        let mut content = tiny_skia::Pixmap::new(1920, 1080).unwrap();
```

This removes the orbs from the thumbnail (they were only animated decoration). If desired, orbs can be re-added as a follow-up by factoring `render_background` to accept explicit dimensions.

- [ ] **Step 2: Confirm `render_thumbnail` still works against a 1920×1080 pixmap**

`render_thumbnail` in `src/render/cards.rs` currently uses the constant `HEIGHT` from config (now 2160). Since the thumbnail canvas is now explicitly 1920×1080, `render_thumbnail` must read dimensions from the pixmap it receives, NOT from `config::HEIGHT`.

In `urantia-render/src/render/cards.rs`, find `render_thumbnail`. Replace any references to the global `HEIGHT` constant inside this function with `pixmap.height() as f32`. Specifically, if the function body has:

```rust
    let h = HEIGHT as f32;
```

replace with:

```rust
    let h = pixmap.height() as f32;
```

Also ensure the `h` used later in the function (for `logo_cy = h / 2.0` and `start_y = (h - total_height) / 2.0`) all read from `pixmap.height()` so the thumbnail layout stays correct at 1920×1080 independently of config.

- [ ] **Step 3: Build**

Run: `cd urantia-render && cargo build 2>&1 | tail -5`
Expected: builds cleanly.

- [ ] **Step 4: Generate a test thumbnail**

Run: `cd urantia-render && cargo run --release -- thumbnail --papers 1 --output-dir output/thumbnails-test`
Expected: writes `output/thumbnails-test/thumbnail-1.png`.

Verify pixel dimensions: `sips -g pixelWidth -g pixelHeight output/thumbnails-test/thumbnail-1.png`
Expected:
```
  pixelWidth: 1920
  pixelHeight: 1080
```

- [ ] **Step 5: Visual check**

Run: `open output/thumbnails-test/thumbnail-1.png`
Expected: same layout as before — logo on left, PAPER 1 in gold, title to the right.

- [ ] **Step 6: Commit**

```bash
cd urantia-render
git add src/main.rs src/render/cards.rs
git commit -m "pin thumbnail canvas to 1920x1080 independent of 4K video canvas"
```

---

### Task 7: Add silence-gap constants in config

**Files:**
- Modify: `urantia-render/src/config.rs` (after existing Timing block)

- [ ] **Step 1: Add the four new constants**

Open `urantia-render/src/config.rs`. Find the block starting with `// Timing (seconds)` (currently around lines 28-34). Add the new constants immediately after `pub const CHUNK_CROSSFADE_SEC: f64 = 0.3;`:

```rust
// Silence between segments (additional to INTRO_PADDING_SEC / SECTION_CARD_PADDING_SEC
// which already pad the end of those cards internally). These create audible breathing
// room in the audio between spoken segments.
pub const GAP_AFTER_INTRO_SEC: f64 = 1.5;          // intro card → first paragraph
pub const GAP_BETWEEN_PARAGRAPHS_SEC: f64 = 0.6;   // paragraph → paragraph (same section)
pub const GAP_BEFORE_SECTION_SEC: f64 = 1.2;       // last paragraph → section card
pub const GAP_AFTER_SECTION_SEC: f64 = 1.0;        // section card → first paragraph
```

Then in the `// Timing (frames)` block (around lines 36-40), add the frame conversions:

```rust
pub const GAP_AFTER_INTRO_FRAMES: u32 = (GAP_AFTER_INTRO_SEC * FPS as f64) as u32;
pub const GAP_BETWEEN_PARAGRAPHS_FRAMES: u32 = (GAP_BETWEEN_PARAGRAPHS_SEC * FPS as f64) as u32;
pub const GAP_BEFORE_SECTION_FRAMES: u32 = (GAP_BEFORE_SECTION_SEC * FPS as f64) as u32;
pub const GAP_AFTER_SECTION_FRAMES: u32 = (GAP_AFTER_SECTION_SEC * FPS as f64) as u32;
```

- [ ] **Step 2: Build**

Run: `cd urantia-render && cargo build 2>&1 | tail -5`
Expected: builds cleanly with four `dead_code` warnings for the new constants (they'll be consumed in Task 8).

- [ ] **Step 3: Commit**

```bash
cd urantia-render
git add src/config.rs
git commit -m "add inter-segment silence gap constants"
```

---

### Task 8: Insert gaps between segments in `build_manifest` (TDD)

**Files:**
- Modify: `urantia-render/src/data/manifest.rs:70-158`
- Add tests inline in the same file.

- [ ] **Step 1: Add a failing test for gap insertion**

Append to the end of `urantia-render/src/data/manifest.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::audio_manifest::AudioManifest;
    use crate::data::paper::{Paper, Section, Paragraph};
    use std::collections::HashMap;

    fn fixture_paper_one_section_two_paragraphs() -> (Paper, AudioManifest) {
        // A minimal Paper with:
        //   - 1 paper intro audio  (2.0 sec)
        //   - Section 0 (no section card rendered)
        //   - Section 1 intro audio (1.5 sec)
        //   - 2 paragraphs in section 1 (3.0 sec each)
        let paper = Paper {
            paper_id: "1".into(),
            part_id: "1".into(),
            paper_title: "Test Paper".into(),
            sections: vec![Section {
                section_id: "1".into(),
                section_title: Some("Test Section".into()),
                paragraphs: vec![
                    Paragraph {
                        global_id: "1:1.1.1".into(),
                        standard_reference_id: "1.1.1".into(),
                        section_title: Some("Test Section".into()),
                        text: "First paragraph text.".into(),
                    },
                    Paragraph {
                        global_id: "1:1.1.2".into(),
                        standard_reference_id: "1.1.2".into(),
                        section_title: Some("Test Section".into()),
                        text: "Second paragraph text.".into(),
                    },
                ],
            }],
        };

        let mut durations = HashMap::new();
        durations.insert("1:1.-.-".into(), 2.0);
        durations.insert("1:1.1.-".into(), 1.5);
        durations.insert("1:1.1.1".into(), 3.0);
        durations.insert("1:1.1.2".into(), 3.0);
        let audio = AudioManifest::from_durations_for_test(durations);

        (paper, audio)
    }

    #[test]
    fn gap_after_intro_is_inserted_before_first_section() {
        let (paper, audio) = fixture_paper_one_section_two_paragraphs();
        let m = build_manifest(&paper, &audio);

        // Intro: 2.0s audio + 1.0s INTRO_PADDING = 3.0s = 90 frames
        // Then GAP_AFTER_INTRO_FRAMES = 45 frames of silence
        // Section card starts at frame 90 + 45 = 135
        let section_start = m.segments.iter().find_map(|s| match s {
            Segment::SectionCard { start_frame, .. } => Some(*start_frame),
            _ => None,
        }).expect("section card missing");
        let expected_intro_frames = (2.0 * FPS as f64).ceil() as u32 + (INTRO_PADDING_SEC * FPS as f64) as u32;
        assert_eq!(section_start, expected_intro_frames + GAP_AFTER_INTRO_FRAMES);
    }

    #[test]
    fn gap_between_paragraphs_is_inserted() {
        let (paper, audio) = fixture_paper_one_section_two_paragraphs();
        let m = build_manifest(&paper, &audio);

        let paragraph_starts: Vec<u32> = m.segments.iter().filter_map(|s| match s {
            Segment::Paragraph { start_frame, .. } => Some(*start_frame),
            _ => None,
        }).collect();
        assert_eq!(paragraph_starts.len(), 2);

        // First paragraph → second paragraph gap = 3.0s audio + GAP_BETWEEN_PARAGRAPHS_FRAMES
        let first_duration_frames = (3.0 * FPS as f64).ceil() as u32;
        assert_eq!(
            paragraph_starts[1],
            paragraph_starts[0] + first_duration_frames + GAP_BETWEEN_PARAGRAPHS_FRAMES
        );
    }

    #[test]
    fn gap_after_section_is_inserted_before_first_paragraph() {
        let (paper, audio) = fixture_paper_one_section_two_paragraphs();
        let m = build_manifest(&paper, &audio);

        let section_end = m.segments.iter().find_map(|s| match s {
            Segment::SectionCard { start_frame, duration_frames, .. } => {
                Some(*start_frame + *duration_frames)
            }
            _ => None,
        }).expect("section card missing");

        let first_paragraph_start = m.segments.iter().find_map(|s| match s {
            Segment::Paragraph { start_frame, .. } => Some(*start_frame),
            _ => None,
        }).expect("first paragraph missing");

        assert_eq!(first_paragraph_start, section_end + GAP_AFTER_SECTION_FRAMES);
    }
}
```

This test references `AudioManifest::from_durations_for_test` and field layouts of `Paper`/`Section`/`Paragraph`. Before running, open `src/data/audio_manifest.rs` and `src/data/paper.rs` — adjust the fixture to match the real struct definitions. If `AudioManifest` doesn't have a test-only constructor, add one in a `#[cfg(test)]` block in `audio_manifest.rs`:

```rust
#[cfg(test)]
impl AudioManifest {
    pub fn from_durations_for_test(durations: std::collections::HashMap<String, f64>) -> Self {
        // Adjust this to match the real internal struct shape.
        AudioManifest { durations }
    }
}
```

- [ ] **Step 2: Run tests — expect failures**

Run: `cd urantia-render && cargo test --lib data::manifest 2>&1 | tail -20`
Expected: all three new tests fail because `build_manifest` does not yet insert gaps.

- [ ] **Step 3: Implement gap insertion in `build_manifest`**

In `urantia-render/src/data/manifest.rs`, modify `build_manifest` so gaps are inserted at the right points. Replace lines 70-158 with:

```rust
pub fn build_manifest(paper: &Paper, audio_manifest: &AudioManifest) -> PaperManifest {
    let mut segments = Vec::new();
    let mut current_frame = 0u32;

    // Intro card
    let paper_global_id = format!("{}:{}.-.-", paper.part_id, paper.paper_id);
    let intro_audio_dur = audio_manifest.get_duration(&paper_global_id);
    let intro_frames = if let Some(dur) = intro_audio_dur {
        (dur * FPS as f64).ceil() as u32 + (INTRO_PADDING_SEC * FPS as f64) as u32
    } else {
        5 * FPS // fallback 5 seconds
    };

    segments.push(Segment::Intro {
        paper_title: paper.paper_title.clone(),
        paper_id: paper.paper_id.clone(),
        start_frame: current_frame,
        duration_frames: intro_frames,
    });
    current_frame += intro_frames;
    // Silence between intro and the first downstream segment.
    current_frame += GAP_AFTER_INTRO_FRAMES;

    // Sections
    for section in &paper.sections {
        let has_section_card = section.section_title.is_some() && section.section_id != "0";

        if has_section_card {
            // Silence before the section card (skip for the first section since
            // GAP_AFTER_INTRO_FRAMES is already applied). Only add if the previous
            // segment pushed was a Paragraph, not the Intro.
            let prev_was_paragraph = matches!(
                segments.last(),
                Some(Segment::Paragraph { .. })
            );
            if prev_was_paragraph {
                current_frame += GAP_BEFORE_SECTION_FRAMES;
            }

            let section_global_id = format!(
                "{}:{}.{}.-",
                paper.part_id, paper.paper_id, section.section_id
            );
            let section_audio_dur = audio_manifest.get_duration(&section_global_id);
            let section_frames = if let Some(dur) = section_audio_dur {
                (dur * FPS as f64).ceil() as u32 + (SECTION_CARD_PADDING_SEC * FPS as f64) as u32
            } else {
                SECTION_CARD_FRAMES
            };

            segments.push(Segment::SectionCard {
                section_title: section.section_title.clone().unwrap_or_default(),
                start_frame: current_frame,
                duration_frames: section_frames,
            });
            current_frame += section_frames;
            // Silence after the section card before its first paragraph.
            current_frame += GAP_AFTER_SECTION_FRAMES;
        }

        // Paragraphs
        let mut first_paragraph_in_section = true;
        for para in &section.paragraphs {
            let duration = match audio_manifest.get_duration(&para.global_id) {
                Some(d) => d,
                None => {
                    eprintln!("Warning: no audio for {}, skipping", para.global_id);
                    continue;
                }
            };

            // Silence between paragraphs within the same section.
            // First paragraph after a section card (or intro) already has its gap
            // applied (GAP_AFTER_SECTION_FRAMES / GAP_AFTER_INTRO_FRAMES) — skip.
            if !first_paragraph_in_section {
                current_frame += GAP_BETWEEN_PARAGRAPHS_FRAMES;
            }
            first_paragraph_in_section = false;

            let duration_frames = (duration * FPS as f64).ceil() as u32;
            let text_chunks = chunk_text(&para.text, duration, duration_frames);

            segments.push(Segment::Paragraph {
                global_id: para.global_id.clone(),
                standard_reference_id: para.standard_reference_id.clone(),
                text: para.text.clone(),
                section_title: para.section_title.clone(),
                audio_duration_sec: duration,
                start_frame: current_frame,
                duration_frames,
                text_chunks,
            });
            current_frame += duration_frames;
        }
    }

    // Outro
    segments.push(Segment::Outro {
        start_frame: current_frame,
        duration_frames: OUTRO_FRAMES,
        tagline: None,
    });
    current_frame += OUTRO_FRAMES;

    PaperManifest {
        paper_id: paper.paper_id.clone(),
        paper_title: paper.paper_title.clone(),
        part_id: paper.part_id.clone(),
        fps: FPS,
        segments,
        total_duration_frames: current_frame,
        total_duration_sec: current_frame / FPS,
    }
}
```

- [ ] **Step 4: Run tests — expect pass**

Run: `cd urantia-render && cargo test --lib data::manifest 2>&1 | tail -10`
Expected: three new tests pass. Existing tests in the module still pass.

- [ ] **Step 5: Run the full test suite**

Run: `cd urantia-render && cargo test 2>&1 | tail -20`
Expected: all tests pass (text_util, data::paper, data::manifest, metadata::youtube).

- [ ] **Step 6: Commit**

```bash
cd urantia-render
git add src/data/manifest.rs src/data/audio_manifest.rs
git commit -m "insert silence gaps between audio segments"
```

---

### Task 9: Re-render one sample video end-to-end to verify 4K + gaps

**Files:** (output only — no code changes)

- [ ] **Step 1: Clear stale cached manifest for paper 1**

Run:
```bash
cd urantia-render
rm -f output/manifests/1.json output/1.mp4
```

(If `output/manifests/` doesn't exist, no-op; the renderer regenerates it.)

- [ ] **Step 2: Regenerate manifest for paper 1**

Run:
```bash
cd urantia-render
cargo run --release -- manifest --papers 1 2>&1 | tail -5
```
Expected: writes `output/manifests/1.json`. Scan the JSON for a paragraph segment and confirm `start_frame` is not equal to `previous_start + previous_duration_frames` (they differ by the gap).

- [ ] **Step 3: Render paper 1 video**

Run:
```bash
cd urantia-render
cargo run --release -- render --papers 1 2>&1 | tail -20
```
Expected: runs ffmpeg, writes `output/1.mp4`. Rendering at 4K takes ~4× as long as 1080p — expect several minutes per paper.

- [ ] **Step 4: Inspect the output**

```bash
ffprobe -v error -select_streams v:0 -show_entries stream=width,height,r_frame_rate output/1.mp4
```
Expected:
```
width=3840
height=2160
r_frame_rate=30/1
```

- [ ] **Step 5: Play a few seconds to verify text legibility and silence gaps**

Run: `open output/1.mp4`
Check at ~2.5s (should hear silence after intro card before paragraph 1), at transitions between paragraphs (0.6s silence), and between a paragraph and a section card (1.2s silence + section card + 1.0s silence).

Verify the paragraph body text reads comfortably at full-screen playback on a monitor.

- [ ] **Step 6: Upload to YouTube as a hidden test (optional but recommended)**

Upload `output/1.mp4` as a private or unlisted video to UrantiaHub and compare pixelation to the original 1080p upload. Let YouTube finish processing all resolutions (~1 hour for 4K) before comparing.

- [ ] **Step 7: If everything looks right, commit nothing (no code change) and move on**

If you want to check in a sample thumbnail of the rendered frame for future regression reference:

```bash
cd urantia-render
ffmpeg -ss 10 -i output/1.mp4 -frames:v 1 -q:v 2 output/samples/paper-1-frame-10s.jpg
git add output/samples/paper-1-frame-10s.jpg
git commit -m "add 4K sample frame from paper 1 for visual regression"
```

(Skip if `output/` is in `.gitignore`.)

---

### Task 10: Batch re-render all 197 papers

**Files:** (output only — no code changes)

This is the heavy-lifting step. Rendering at 4K will take multiple hours end-to-end. Plan to run this overnight or in a tmux session.

- [ ] **Step 1: Confirm audio cache is present (avoid re-downloading)**

Run: `ls urantia-render/output/audio/*.mp3 | wc -l`
Expected: a lot — one per paragraph intro, section intro, paragraph. If low, the first render command re-downloads automatically.

- [ ] **Step 2: Clear all stale manifests and rendered videos**

Run:
```bash
cd urantia-render
rm -f output/manifests/*.json output/*.mp4
```

- [ ] **Step 3: Regenerate all manifests**

Run:
```bash
cd urantia-render
cargo run --release -- manifest --papers 0-196 2>&1 | tail -5
```
Expected: 197 manifest JSON files in `output/manifests/`.

- [ ] **Step 4: Render all 197 papers**

Run:
```bash
cd urantia-render
nohup cargo run --release -- render --papers 0-196 > output/render-4k.log 2>&1 &
echo "render pid: $!"
```
Then monitor: `tail -f output/render-4k.log` — you'll see per-paper progress. Expect the full batch to take a lot of hours.

Check periodically: `ls output/*.mp4 | wc -l` — should grow toward 197.

- [ ] **Step 5: Regenerate metadata JSON + upload sheets**

Metadata doesn't change with resolution — but if the cached manifests fed `generate_metadata` before Task 8, they had no gaps, so durations are slightly off. Regenerate:

```bash
cd urantia-render
cargo run --release -- metadata --papers 0-196 2>&1 | tail -5
```
Expected: 197 JSON files in `output/` + 197 markdown sheets in `output/sheets/`.

- [ ] **Step 6: Spot-check three videos for sanity**

`open output/0.mp4 output/42.mp4 output/196.mp4`

Verify each one:
- Plays at 4K.
- Has audible silence gaps between paragraphs.
- Has a longer gap after the paper title intro.
- Has noticeable gaps on either side of section cards.
- Text is legible.

- [ ] **Step 7: Commit nothing (output artifacts)**

Typically `output/` is gitignored; no commit needed. If it's not, add the produced MP4s and metadata in one commit.

---

### Task 11: Redesign playlist thumbnails + add `playlist-thumbnails` CLI subcommand

**Files:**
- Modify: `urantia-render/src/render/cards.rs` — rewrite `render_playlist_thumbnail_with_subtitle` with the new logo-left + text-right aesthetic matching per-paper thumbnails.
- Modify: `urantia-render/src/main.rs` — add a `PlaylistThumbnails` subcommand + handler.
- Modify: `urantia-render/src/render/frame.rs:179-216` — remove the old `test_render_playlist_thumbnails` test that currently renders them; the new CLI subcommand replaces it.

Current state: playlist thumbnails are generated only via a hidden unit test in `frame.rs` that writes 5 PNGs (`playlist-all.png`, `playlist-part-1.png`…`-4.png`) using the old centered "label over title" design with no logo. When the user uploads the Part I–IV playlists + "All Papers" master playlist, these thumbnails need to match the cohesive branding we just built for individual papers.

Target design: same as the paper thumbnail — UrantiaHub logo on the left, gold label (`PART I`, `PART II`, etc.) on the right over the part title in white. For the master "all papers" playlist the label slot shows "THE URANTIA PAPERS" in gold and "All 197, audio and text." in white below.

- [ ] **Step 1: Rewrite `render_playlist_thumbnail_with_subtitle`**

In `urantia-render/src/render/cards.rs`, replace the entire body of `render_playlist_thumbnail_with_subtitle` (lines 17-57) with:

```rust
pub fn render_playlist_thumbnail_with_subtitle(
    renderer: &mut TextRenderer,
    pixmap: &mut Pixmap,
    label: &str,
    title: &str,
    subtitle: Option<&str>,
) {
    // Playlist thumbnails ship at 1920×1080 (YouTube browse assets), same as
    // per-paper thumbnails. Read dimensions from the pixmap so the layout stays
    // correct regardless of the video config canvas size.
    let h = pixmap.height() as f32;

    // Logo on the left.
    let logo_cx = 380.0;
    let logo_cy = h / 2.0;
    let logo_radius = 290.0;
    render_concentric_logo(pixmap, logo_cx, logo_cy, logo_radius);

    // Text column on the right.
    let text_x = 760.0;
    let text_max_width = 1100.0;
    let gap = 40.0;

    let label_upper = label.to_uppercase();
    let label_or_master = if label.is_empty() {
        "THE URANTIA PAPERS".to_string()
    } else {
        label_upper
    };

    let label_measure = TextStyle::thumbnail_paper_number(text_x, 0.0, text_max_width);
    let label_height = renderer.measure_text(&label_or_master, &label_measure);

    let title_measure = TextStyle::thumbnail_paper_title_right(text_x, 0.0, text_max_width);
    let title_height = renderer.measure_text(title, &title_measure);

    let subtitle_height = subtitle
        .map(|s| renderer.measure_text(s, &title_measure))
        .unwrap_or(0.0);
    let subtitle_gap = if subtitle.is_some() { gap } else { 0.0 };

    let total_height = label_height + gap + title_height + subtitle_gap + subtitle_height;
    let start_y = (h - total_height) / 2.0;

    let label_style = TextStyle::thumbnail_paper_number(text_x, start_y, text_max_width);
    renderer.render_text(pixmap, &label_or_master, &label_style);

    let title_y = start_y + label_height + gap;
    let title_style = TextStyle::thumbnail_paper_title_right(text_x, title_y, text_max_width);
    renderer.render_text(pixmap, title, &title_style);

    if let Some(sub) = subtitle {
        let sub_y = title_y + title_height + subtitle_gap;
        let sub_style = TextStyle::thumbnail_paper_title_right(text_x, sub_y, text_max_width);
        renderer.render_text(pixmap, sub, &sub_style);
    }
}
```

- [ ] **Step 2: Remove the obsolete test**

In `urantia-render/src/render/frame.rs`, delete the `test_render_playlist_thumbnails` function (lines 179-216 approximately). It was the de-facto "render command" for playlists; the new CLI subcommand replaces it.

- [ ] **Step 3: Add the CLI subcommand**

In `urantia-render/src/main.rs`, add a new variant to the `Commands` enum (insert near `Banner` and `ChannelIcon`):

```rust
    /// Render the 5 YouTube playlist thumbnails (master + Parts I-IV)
    PlaylistThumbnails {
        #[arg(long, default_value = "./output/thumbnails")]
        output_dir: PathBuf,
    },
```

Add the match arm alongside the other subcommand dispatches:

```rust
        Commands::PlaylistThumbnails { output_dir } => cmd_playlist_thumbnails(output_dir).await?,
```

Add the handler function alongside `cmd_banner` and `cmd_channel_icon`:

```rust
async fn cmd_playlist_thumbnails(output_dir: &PathBuf) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;

    println!("Rendering 5 playlist thumbnails (1920x1080)...");

    let mut renderer = render::text::TextRenderer::new();

    // Build a dark-fill 1920x1080 canvas (same approach as per-paper thumbnails).
    let build_canvas = || -> tiny_skia::Pixmap {
        let mut pixmap = tiny_skia::Pixmap::new(1920, 1080).unwrap();
        {
            let data = pixmap.data_mut();
            for i in (0..data.len()).step_by(4) {
                data[i]     = config::BG_COLOR[0];
                data[i + 1] = config::BG_COLOR[1];
                data[i + 2] = config::BG_COLOR[2];
                data[i + 3] = config::BG_COLOR[3];
            }
        }
        pixmap
    };

    // Master playlist (all 197)
    {
        let mut pixmap = build_canvas();
        render::cards::render_playlist_thumbnail_with_subtitle(
            &mut renderer,
            &mut pixmap,
            "",
            "All 197 Papers",
            Some("Audio and text, read along"),
        );
        let out = output_dir.join("playlist-all.png");
        pixmap.save_png(&out)?;
        println!("  → {}", out.display());
    }

    // Parts I–IV
    let parts = [
        ("Part I",   "The Central and\nSuperuniverses", "playlist-part-1"),
        ("Part II",  "The Local Universe",              "playlist-part-2"),
        ("Part III", "The History\nof Urantia",         "playlist-part-3"),
        ("Part IV",  "The Life and Teachings\nof Jesus","playlist-part-4"),
    ];
    for (label, title, file_stem) in parts.iter() {
        let mut pixmap = build_canvas();
        render::cards::render_playlist_thumbnail(&mut renderer, &mut pixmap, label, title);
        let out = output_dir.join(format!("{}.png", file_stem));
        pixmap.save_png(&out)?;
        println!("  → {}", out.display());
    }

    println!("Done!");
    Ok(())
}
```

- [ ] **Step 4: Build**

Run: `cd urantia-render && cargo build --release 2>&1 | tail -5`
Expected: builds cleanly. The `render_playlist_thumbnail` / `render_playlist_thumbnail_with_subtitle` warnings about "never used" should disappear now that the CLI consumes them.

- [ ] **Step 5: Generate the five thumbnails**

Run:
```bash
cd urantia-render
cargo run --release -- playlist-thumbnails --output-dir output/thumbnails 2>&1 | tail -10
```
Expected: 5 PNGs written:
- `output/thumbnails/playlist-all.png`
- `output/thumbnails/playlist-part-1.png`
- `output/thumbnails/playlist-part-2.png`
- `output/thumbnails/playlist-part-3.png`
- `output/thumbnails/playlist-part-4.png`

- [ ] **Step 6: Visual inspection**

Run: `open output/thumbnails/playlist-all.png output/thumbnails/playlist-part-1.png`

Check each:
- Logo visible on the left
- Label in gold on the right (`PART I` for parts, `THE URANTIA PAPERS` for master)
- Title readable in white below the label
- Multi-line titles wrap cleanly (Parts I, III, IV have an embedded `\n`)
- Subtitle (master only) reads below the title in white

If any of the Part titles look cramped, reduce `thumbnail_paper_title_right` font_size locally for playlists via a new TextStyle preset — but try the default first; the layout matches per-paper thumbnails exactly and should fit.

- [ ] **Step 7: Verify dimensions**

```bash
for f in output/thumbnails/playlist-*.png; do
  sips -g pixelWidth -g pixelHeight "$f" | grep -E "pixelWidth|pixelHeight"
done
```
Expected: every file is 1920×1080.

- [ ] **Step 8: Commit**

```bash
cd urantia-render
git add src/render/cards.rs src/render/frame.rs src/main.rs
git commit -m "add playlist-thumbnails subcommand with logo + label + title layout"
```

- [ ] **Step 9: Update the upload guide**

Open `urantia-render/YOUTUBE_UPLOAD_GUIDE.md`. In the Channel Setup Checklist (near the bottom), add a line under the existing playlist entry:

```markdown
- [ ] Playlist thumbnails: generate all 5 with `urantia-render playlist-thumbnails`
  - `output/thumbnails/playlist-all.png` for the master "All Papers" playlist
  - `output/thumbnails/playlist-part-1.png` through `-4.png` for the per-part playlists
```

- [ ] **Step 10: Commit the guide update**

```bash
cd urantia-render
git add YOUTUBE_UPLOAD_GUIDE.md
git commit -m "document playlist-thumbnails workflow in upload guide"
```

---

## Verification

1. **Build** — `cd urantia-render && cargo build --release` succeeds cleanly.
2. **Tests** — `cd urantia-render && cargo test` passes including the new manifest-gap tests.
3. **Resolution check** — `ffprobe` on any rendered video shows 3840×2160.
4. **Gap audit** — Open a manifest JSON and confirm adjacent paragraph start_frames differ by `first_duration + GAP_BETWEEN_PARAGRAPHS_FRAMES` (not just first_duration).
5. **Thumbnail preserved** — `sips -g pixelWidth output/thumbnails/thumbnail-1.png` returns 1920.
6. **Banner preserved** — `sips -g pixelWidth output/banner.png` returns 2560.
7. **Metadata durations updated** — `cat output/1.json | python3 -c "import json,sys;print(json.load(sys.stdin)['duration_sec'])"` returns a slightly larger number than before gaps were added.

## Out of Scope (for future plans)

- Updating existing YouTube uploads to the new videos + titles/thumbnails. Separate plan.
- Adding orbs to the thumbnail canvas post-4K split (Task 6 drops them for simplicity).
- Audio crossfades at gap boundaries. Current plan uses hard cuts; if abrupt start/stop of each clip is noticeable, a follow-up can add ~50ms cosine ramps in `audio/concat.rs`.
