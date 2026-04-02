# urantia-render

High-performance Rust video renderer for the UrantiaHub YouTube channel. Renders 197 Urantia Paper videos with pre-existing TTS audio.

## Tech Stack

- Language: Rust (edition 2021, requires 1.86+)
- 2D rendering: tiny-skia (CPU rasterizer)
- Text layout: cosmic-text (paragraph wrapping + font shaping)
- Video encoding: ffmpeg (spawned as subprocess, raw frames piped via stdin)
- Audio decode: symphonia (pure Rust MP3 decoder)
- HTTP: reqwest + tokio
- S3/R2: rust-s3
- CLI: clap (derive)
- Parallelism: rayon

## Structure

- `src/main.rs` — CLI entry point with clap subcommands (download, manifest, render, metadata, upload, all)
- `src/config.rs` — All constants: dimensions, colors, timing, CDN URLs, text chunking thresholds
- `src/data/` — Data pipeline
  - `paper.rs` — Paper/Section/Paragraph types, JSON deserialization from CDN
  - `audio_manifest.rs` — Audio manifest loader (globalId → duration lookup)
  - `text_chunker.rs` — Duration-aware text splitting at sentence boundaries
  - `manifest.rs` — PaperManifest builder (segments with frame ranges)
- `src/render/` — Frame rendering (Phase 2, stubs)
  - `background.rs` — 3 gradient orbs with trig animation
  - `text.rs` — cosmic-text layout → tiny-skia glyph rasterization
  - `cards.rs` — Intro, section, outro card layouts
  - `compositor.rs` — Alpha-blend layers with fade opacity
  - `frame.rs` — Full frame generation
  - `pipeline.rs` — Keyframe+repeat frame loop feeding ffmpeg
- `src/audio/` — Audio handling
  - `download.rs` — Async CDN download with concurrency + resume
  - `concat.rs` — symphonia MP3 decode, PCM buffer assembly (Phase 3, stub)
- `src/encode/ffmpeg.rs` — Spawn ffmpeg, pipe video + audio (Phase 3, stub)
- `src/upload/r2.rs` — S3-compatible R2 upload (Phase 4, stub)
- `src/metadata/youtube.rs` — YouTube metadata generation (Phase 4, stub)
- `assets/fonts/` — TTF files (Source Serif 4, DM Sans) — not yet downloaded

## Commands

- `cargo build --release` — Build optimized binary
- `cargo run -- download --papers 1` — Download audio from CDN
- `cargo run -- manifest --papers 1 --manifest-path <path>` — Build timing manifest
- `cargo run -- render --papers 1` — Render video (not yet implemented)

## Key Architecture Decisions

- **Keyframe + repeat**: During paragraph hold phases, render 1 frame/sec and repeat each 30x to ffmpeg. Reduces unique renders from ~70K to ~4.5K per paper.
- **PCM assembly**: Decode all MP3s with symphonia, write PCM samples at exact offsets in a single buffer. Pipe to ffmpeg alongside video. Gives sample-accurate sync.
- **ffmpeg via subprocess**: Avoids ffmpeg-sys build complexity. Raw RGBA frames piped via stdin.
- **CDN-first data loading**: Paper JSON and audio manifest fetched from cdn.urantia.dev. Local file fallback via `--manifest-path`.

## Audio CDN

- Paper audio: `https://audio.urantia.dev/tts-1-hd-nova-{globalId}.mp3`
- Paper intro: globalId `{partId}:{paperId}.-.-` (e.g., `1:1.-.-`)
- Section intro: globalId `{partId}:{paperId}.{sectionId}.-` (e.g., `1:1.1.-`)
- Manifest: `https://cdn.urantia.dev/manifests/audio-manifest.json`

## Notes

- Audio manifest from CDN has slightly different key behavior than local copy — use `--manifest-path` with local `urantia-dev-api/data/audio-manifest.json` for reliable lookups
- Phase 2+ modules are stubs (compile but no implementation)
- Fonts not yet downloaded into `assets/fonts/`
