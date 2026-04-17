// Video — render at 4K so YouTube's per-resolution bitrate tier has enough
// budget to keep gray text on dark backgrounds from pixelating.
pub const WIDTH: u32 = 3840;
pub const HEIGHT: u32 = 2160;
pub const FPS: u32 = 30;

/// Multiplier for any pixel dimension designed at the 1920×1080 reference size.
/// Used to scale font sizes, orb radii, padding, etc.
pub const RESOLUTION_SCALE: f32 = WIDTH as f32 / 1920.0;

// CDN
pub const AUDIO_CDN_BASE: &str = "https://audio.urantia.dev";
pub const PAPER_CDN_BASE: &str = "https://cdn.urantia.dev/json/eng";
pub const MANIFEST_CDN_URL: &str = "https://cdn.urantia.dev/manifests/audio-manifest.json";
pub const AUDIO_MODEL: &str = "tts-1-hd";
pub const AUDIO_VOICE: &str = "nova";

// R2
pub const R2_BUCKET: &str = "urantiahub-video";

// Colors (RGBA 0-255)
pub const BG_COLOR: [u8; 4] = [10, 10, 15, 255]; // #0a0a0f
pub const TEXT_COLOR: [u8; 4] = [232, 230, 225, 255]; // #e8e6e1
// Muted text: rgba(232,230,225,0.6) composited on #0a0a0f → solid RGB.
// Previously 0.4 alpha (#636263) got crushed by YouTube VP9 reencoding.
// R: 10 * 0.4 + 232 * 0.6 = 143, G: 10 * 0.4 + 230 * 0.6 = 142, B: 15 * 0.4 + 225 * 0.6 = 141
pub const TEXT_MUTED: [u8; 4] = [143, 142, 141, 255];

// Glow orb colors (RGBA, premultiplied alpha)
pub const GOLD_GLOW: [f32; 4] = [186.0 / 255.0, 117.0 / 255.0, 23.0 / 255.0, 0.08];
pub const BLUE_GLOW: [f32; 4] = [60.0 / 255.0, 60.0 / 255.0, 180.0 / 255.0, 0.06];
pub const PURPLE_GLOW: [f32; 4] = [120.0 / 255.0, 60.0 / 255.0, 160.0 / 255.0, 0.05];

// Timing (seconds)
pub const INTRO_PADDING_SEC: f64 = 1.0; // extra time after intro audio
pub const OUTRO_SEC: f64 = 5.0;
pub const SECTION_CARD_SEC: f64 = 3.0;
pub const SECTION_CARD_PADDING_SEC: f64 = 1.0;
pub const FADE_SEC: f64 = 0.5;
pub const CHUNK_CROSSFADE_SEC: f64 = 0.3;

// Silence between segments (additional to INTRO_PADDING_SEC / SECTION_CARD_PADDING_SEC
// which pad the end of those cards internally). These create audible breathing room
// in the audio between spoken segments.
pub const GAP_AFTER_INTRO_SEC: f64 = 1.5;         // intro card → first paragraph
pub const GAP_BETWEEN_PARAGRAPHS_SEC: f64 = 0.6;  // paragraph → paragraph (same section)
pub const GAP_BEFORE_SECTION_SEC: f64 = 1.2;      // last paragraph → section card
pub const GAP_AFTER_SECTION_SEC: f64 = 1.0;       // section card → first paragraph

// Timing (frames)
pub const OUTRO_FRAMES: u32 = (OUTRO_SEC * FPS as f64) as u32;
pub const SECTION_CARD_FRAMES: u32 = (SECTION_CARD_SEC * FPS as f64) as u32;
pub const FADE_FRAMES: u32 = (FADE_SEC * FPS as f64) as u32;
pub const CHUNK_CROSSFADE_FRAMES: u32 = (CHUNK_CROSSFADE_SEC * FPS as f64) as u32;
pub const GAP_AFTER_INTRO_FRAMES: u32 = (GAP_AFTER_INTRO_SEC * FPS as f64) as u32;
pub const GAP_BETWEEN_PARAGRAPHS_FRAMES: u32 = (GAP_BETWEEN_PARAGRAPHS_SEC * FPS as f64) as u32;
pub const GAP_BEFORE_SECTION_FRAMES: u32 = (GAP_BEFORE_SECTION_SEC * FPS as f64) as u32;
pub const GAP_AFTER_SECTION_FRAMES: u32 = (GAP_AFTER_SECTION_SEC * FPS as f64) as u32;

// Text chunking
pub const MIN_AUDIO_DURATION_FOR_SPLIT: f64 = 15.0;
pub const MIN_CHARS_FOR_SPLIT: usize = 400;
pub const MIN_CHUNK_CHARS: usize = 100;

// Download concurrency
pub const DOWNLOAD_CONCURRENCY: usize = 10;

pub fn audio_url(global_id: &str) -> String {
    format!("{}/{}-{}-{}.mp3", AUDIO_CDN_BASE, AUDIO_MODEL, AUDIO_VOICE, global_id)
}

pub fn paper_cdn_url(paper_id: &str) -> String {
    format!("{}/{}.json", PAPER_CDN_BASE, format!("{:0>3}", paper_id))
}

pub fn video_filename(paper_id: &str) -> String {
    format!("{}-{}-{}.mp4", AUDIO_MODEL, AUDIO_VOICE, paper_id)
}
