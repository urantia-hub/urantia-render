# AWS Spot Batch Cost Validation Prompt

Paste this into Claude Desktop (or any deep-research model) to get an independent second opinion on cost + timing before committing.

---

I need you to validate an AWS EC2 spot instance cost + timing estimate for a CPU-bound batch video rendering job. The estimate I was given is:

> `c7i.16xlarge` spot (~$0.60-1.20/hr), concurrency 8, full batch finishes in ~8 hours, total cost ~$5-15 USD.

Please research current AWS spot pricing, typical spot availability in common regions, and whether the workload characteristics match the instance family, then produce a more confident estimate with reasoning.

## Workload details

**Project:** `urantia-render`, a Rust CLI that produces 197 YouTube videos from pre-existing TTS MP3 audio + structured JSON text data. Each video is 30-60 min of 4K (3840×2160) content — read-along paragraphs with minimal motion (animated gradient orbs + static text cards + 1 fade per paragraph). Production-quality output for https://youtube.com/@UrantiaHub.

**Per-paper workload breakdown (measured on an M1 Pro MacBook):**

- **Rust rasterization** (single-threaded per paper, CPU-only, `tiny-skia` crate):
  - ~4,500 unique "keyframes" rendered per paper (the `--keyframe+repeat` optimization holds each unique frame for 3 video frames during paragraph reads). This runs at ~1-2 frames/sec on a single M1 Pro core.
  - Text rasterization via `cosmic-text` for paragraph layout.
  - Per-frame output: 3840×2160 RGBA = 33 MB raw buffer, piped via stdin to ffmpeg.

- **Encoding:** currently `h264_videotoolbox` (Apple hardware encoder) on Mac, quality `-q:v 65`, profile high. On Linux x86 we'd fall back to `libx264 -preset fast -crf 20` since there's no ARM Media Engine.

- **Per-paper data flow:** feed ~72,000 frames (30 fps × ~40 min avg) to ffmpeg as raw RGBA. Total bytes piped per paper: ~2.3 TB (mostly repeated frames due to keyframe optimization — ffmpeg encodes these fast since motion vectors are zero).

- **Parallelism:** the pipeline already supports `--concurrency N` via rayon. N papers process in parallel, each with its own Rust rasterizer + ffmpeg process.

**Measured timing on M1 Pro (10-core, 8 perf cores, 16 GB RAM):**

- Single paper at concurrency 1 with VideoToolbox hardware encode: **~50 min per paper**.
- Rust side consumes ~1 core at ~80% utilization; ffmpeg VideoToolbox consumes ~50% CPU.
- Full 197-paper batch projection on M1 Pro: ~165 hours sequential, or ~100 hours at concurrency 2 (contending for single Media Engine). Not viable locally.

**On x86 Linux:**

- No hardware encoder, so libx264 software encode. At 4K `-preset fast` on 8 cores, rough throughput for low-entropy gray-text-on-dark content is probably ~10-15 fps. That's ~2x slower than real-time = ~80 min encode per 40-min paper.
- Rust rasterization speed depends on single-core clock. AWS `c7i` Sapphire Rapids Xeon has decent single-thread perf; rasterization would likely complete in ~30-40 min per paper on a single core (slower than M1's per-core performance).
- **The BOTTLENECK question:** which is slower on x86 — Rust rasterization or libx264 encode? Both are roughly 30-80 min per paper per core-pool. Running them in a pipeline, the slowest stage dominates.

## The specific claim to validate

> c7i.16xlarge (64 vCPU, 128 GB RAM) at concurrency 8: each of 8 parallel papers gets ~8 cores, finishes in ~20-30 min per paper. Throughput: 8 papers per ~25 min = 197 papers in ~8 hours. Cost: ~$1/hr spot × 8 hours = ~$8.

Questions to answer with research + reasoning:

1. **Current c7i.16xlarge spot price** in us-east-1, us-east-2, us-west-2 as of today. Expect it to be in the $0.50-1.50/hr range but verify. Any cheaper regions? Note the on-demand baseline (~$2.86/hr) for comparison.

2. **Spot interruption risk** for c7i.16xlarge — is it frequently interrupted? If yes, is there a `urantia-render render --skip-existing` workflow that makes interruption safe (restarts only unfinished papers). There is — the pipeline checks for existing MP4s by size and skips them.

3. **Is `c7i.16xlarge` the right family?** Would `m7i.16xlarge` (more RAM, same vCPU) be better given the 33 MB frame buffers? What about `c7a.16xlarge` (AMD EPYC) — typically cheaper spot with similar single-thread perf for this workload?

4. **Is concurrency 8 the right choice?** Each parallel paper uses roughly 6-8 cores between Rust (1) + ffmpeg threads (5-7). Running 8 in parallel on 64 vCPU means each gets ~8 cores. More concurrency means less per-paper CPU and probably longer per-paper but more total throughput. Where's the sweet spot?

5. **libx264 vs NVENC on a GPU instance** — would a `g5.xlarge` with an NVIDIA A10G and NVENC be faster or cheaper? Hardware H.264 encoding is ~10x faster than libx264, but the rasterization bottleneck might make this moot.

6. **Storage + network egress:** output is ~750 MB per paper × 197 = ~150 GB. Need to rsync back to my laptop after. EBS storage while running is negligible (<$0.10/hr for 200 GB gp3). Egress at ~$0.09/GB × 150 GB = $13.50 — this might actually exceed the compute cost.

7. **Estimated total cost and total time**, factoring in:
   - Spot compute
   - EBS storage
   - Data transfer back to laptop
   - Any setup/teardown overhead

Please produce a final recommended strategy (instance type, region, concurrency, whether GPU is worth it) with total cost and duration estimates, with sources/reasoning visible.

---

**Additional context (for web search):**

- Rust cargo binary is platform-portable. Would cross-compile on my M1 to x86-64-linux via `cargo build --release --target x86_64-unknown-linux-gnu` or build fresh on the spot instance.
- Source code: https://github.com/urantia-hub/urantia-render (public)
- Actual project file paths relevant: `src/render/pipeline.rs` for the frame loop, `src/encode/ffmpeg.rs` for encoding config, `src/main.rs:374+` for the parallel render command (uses rayon).
- The ffmpeg flags I'd use on Linux (no VideoToolbox):
  ```
  -c:v libx264 -preset fast -crf 20 -pix_fmt yuv420p
  ```
- Audio is pre-downloaded MP3s (~150 MB total for 197 papers), decoded with symphonia in the Rust pipeline and written as one WAV per paper to /tmp. Not a concern for bandwidth/disk.
