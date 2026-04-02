# urantia-render — TODO

## Done

- [x] Cargo project scaffold with all dependencies
- [x] CLI with clap (download, manifest, render, metadata, upload, all subcommands)
- [x] config.rs — all constants, colors, timing, CDN URLs
- [x] Paper JSON parser (from CDN or local file)
- [x] Audio manifest loader (globalId → duration lookup)
- [x] Duration-aware text chunker (sentence splitting, proportional frame allocation)
- [x] Manifest builder (segments with frame ranges, matches TypeScript output)
- [x] Async audio download from CDN with concurrency + resume
- [x] README, CLAUDE.md, .env.example

## Phase 2 — Frame Rendering

- [ ] Download font TTF files (Source Serif 4 Light/Regular, DM Sans Regular)
- [ ] Background renderer — 3 gradient orbs with trig animation (tiny-skia)
- [ ] Text renderer — cosmic-text paragraph layout → tiny-skia glyph rasterization
- [ ] Card layouts — intro, section, outro
- [ ] Compositor — alpha-blend text onto background, apply fade opacity
- [ ] Frame generator — given segment + frame offset → 1920x1080 RGBA buffer
- [ ] Dump test frames as PNG for visual comparison with Remotion

## Phase 3 — Encoding Pipeline

- [ ] Audio concat — decode MP3s with symphonia, assemble single PCM buffer
- [ ] ffmpeg subprocess — pipe raw video + PCM audio → MP4
- [ ] Render pipeline — keyframe+repeat frame loop feeding ffmpeg
- [ ] `render` CLI subcommand with --preview and --skip-existing
- [ ] Render Paper 1, verify audio sync + visual quality

## Phase 4 — Upload + Metadata

- [ ] R2 upload (rust-s3, S3-compatible)
- [ ] YouTube metadata generator (title, description, chapters, tags, playlist manifest)
- [ ] `upload` and `metadata` CLI subcommands

## Phase 5 — Parallelism + Full Run

- [ ] rayon thread pool for multi-paper rendering
- [ ] indicatif progress bars
- [ ] Error recovery (skip failed, continue)
- [ ] --concurrency flag
- [ ] `all` CLI subcommand (full pipeline)
- [ ] Render all 197 papers, spot-check
- [ ] Target: all papers in under 3 hours
