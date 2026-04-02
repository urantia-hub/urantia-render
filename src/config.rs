// Video
pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;
pub const FPS: u32 = 30;

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
pub const TEXT_MUTED: [u8; 4] = [232, 230, 225, 102]; // rgba(232,230,225,0.4)

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

// Timing (frames)
pub const OUTRO_FRAMES: u32 = (OUTRO_SEC * FPS as f64) as u32;
pub const SECTION_CARD_FRAMES: u32 = (SECTION_CARD_SEC * FPS as f64) as u32;
pub const FADE_FRAMES: u32 = (FADE_SEC * FPS as f64) as u32;
pub const CHUNK_CROSSFADE_FRAMES: u32 = (CHUNK_CROSSFADE_SEC * FPS as f64) as u32;

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
