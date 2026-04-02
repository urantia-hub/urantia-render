# urantia-render

High-performance Rust video renderer for the UrantiaHub YouTube channel. Generates 197 Urantia Paper videos with TTS narration — targeting ~20x faster than the Remotion pipeline.

## Architecture

Renders frames with `tiny-skia` (CPU rasterizer) + `cosmic-text` (text layout), pipes raw RGBA frames to `ffmpeg` for H.264 encoding, and decodes MP3 audio with `symphonia` for sample-accurate PCM assembly. A "keyframe + repeat" optimization reduces unique frame renders from ~70,000 to ~4,500 per paper.

## Tech Stack

| Concern | Crate |
|---------|-------|
| 2D rendering | `tiny-skia` |
| Text layout | `cosmic-text` |
| Video encoding | `ffmpeg` (subprocess, piped) |
| Audio decode | `symphonia` |
| HTTP | `reqwest` + `tokio` |
| S3/R2 upload | `rust-s3` |
| CLI | `clap` |
| Parallelism | `rayon` |

## Prerequisites

- Rust 1.86+ (`rustup update stable`)
- `ffmpeg` installed (`brew install ffmpeg`)
- Audio manifest from `urantia-dev-api/data/audio-manifest.json` (or fetched from CDN)

## Usage

```bash
# Build release binary
cargo build --release

# Download audio for papers 1-5
cargo run -- download --papers 1-5

# Build timing manifests
cargo run -- manifest --papers 1-5 --manifest-path /path/to/audio-manifest.json

# Render videos (Phase 2-3, not yet implemented)
cargo run -- render --papers 1-5

# Generate YouTube metadata (Phase 4, not yet implemented)
cargo run -- metadata --papers 1-5

# Upload to R2 (Phase 4, not yet implemented)
cargo run -- upload --papers 1-5

# Full pipeline
cargo run -- all --papers 0-196
```

## Audio Source

All audio is pre-generated and hosted on the CDN:

- **URL pattern**: `https://audio.urantia.dev/tts-1-hd-nova-{globalId}.mp3`
- **Coverage**: 16,219 paragraphs (99.99%)
- **Voice**: OpenAI TTS `tts-1-hd` model, `nova` voice

Paper/section intro audio also available (e.g., `1:1.-.-` says "Paper 1: The Universal Father").

## Video Design

- **Resolution**: 1920x1080, 30fps
- **Background**: Dark (#0a0a0f) with animated gold/blue/purple gradient orbs
- **Typography**: Source Serif 4 (text), DM Sans (labels)
- **Transitions**: 0.5s fade in/out per paragraph
- **Intro card**: Paper number + title with narrated audio
- **Section cards**: Section title with narrated audio (~3s)
- **Outro card**: UrantiaHub branding (5s)

## Storage

- **R2 bucket**: `urantiahub-video`
- **Domains**: `video.urantia.dev`, `video.urantiahub.com`
- **URL pattern**: `https://video.urantia.dev/tts-1-hd-nova-{paperId}.mp4`

## Performance Target

| Approach | Per paper | All 197 |
|----------|----------|---------|
| Remotion (current) | ~30 min | ~4-5 days |
| Rust, sequential | ~90-120s | ~5-7 hours |
| Rust, 4 parallel | ~90-120s | ~1.5-2 hours |

## Implementation Status

- [x] Phase 1: Data pipeline + CLI scaffold
- [ ] Phase 2: Frame rendering (tiny-skia + cosmic-text)
- [ ] Phase 3: Encoding pipeline (ffmpeg + symphonia PCM assembly)
- [ ] Phase 4: Upload + metadata generation
- [ ] Phase 5: Parallelism + full run
