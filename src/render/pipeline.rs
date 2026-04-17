use anyhow::Result;
use std::path::Path;

use crate::config::{FADE_FRAMES, FPS};
use crate::data::manifest::PaperManifest;
use crate::encode::ffmpeg::FfmpegEncoder;
use crate::render::frame::render_frame;
use crate::render::text::TextRenderer;

/// Render a complete paper video: generate every frame and pipe to ffmpeg.
/// Every frame is rendered uniquely for smooth glow animation.
/// If max_frames is Some(n), only render the first n frames (for testing).
pub fn render_paper(
    manifest: &PaperManifest,
    output_path: &Path,
    audio_wav_path: &Path,
    max_frames: Option<u32>,
) -> Result<()> {
    let mut renderer = TextRenderer::new();
    let mut encoder = FfmpegEncoder::new(output_path, audio_wav_path)?;

    let total_frames = manifest.total_duration_frames;
    let mut frames_written = 0u32;
    let mut prev_end_frame = 0u32;

    for segment in &manifest.segments {
        let start_frame = segment.start_frame();
        let duration = segment.duration_frames();

        // Emit background-only frames for any silence gap before this segment
        // (gaps come from GAP_AFTER_INTRO / GAP_BETWEEN_PARAGRAPHS /
        //  GAP_BEFORE_SECTION / GAP_AFTER_SECTION in build_manifest).
        // Without this, the video is shorter than the audio and ffmpeg's
        // -shortest flag truncates the audio, causing sync drift and cutoff.
        if start_frame > prev_end_frame {
            let mut gf = prev_end_frame;
            while gf < start_frame {
                if max_frames.is_some_and(|m| frames_written >= m) { break; }
                let global_time = gf as f64 / FPS as f64;
                let pixmap = crate::render::background::render_background(global_time);
                let frame_data = pixmap.data();
                let repeat = 3u32.min(start_frame - gf);
                for _ in 0..repeat {
                    if max_frames.is_some_and(|m| frames_written >= m) { break; }
                    encoder.write_frame(frame_data)?;
                    frames_written += 1;
                }
                gf += repeat;
            }
        }

        if duration == 0 {
            prev_end_frame = start_frame;
            continue;
        }

        // Render with keyframe+repeat optimization:
        // - Fade in/out: render every frame (opacity changes)
        // - Hold: render every 3 frames, repeat 3x (10fps glow = smooth enough)
        let fade = FADE_FRAMES.min(duration / 2);
        let hold_start = fade;
        let hold_end = duration.saturating_sub(fade);
        let hold_step = 3u32; // render every 3rd frame during hold

        // Fade in
        for local_frame in 0..hold_start {
            if max_frames.is_some_and(|m| frames_written >= m) { break; }
            let global_time = (start_frame + local_frame) as f64 / FPS as f64;
            let pixmap = render_frame(&mut renderer, segment, local_frame, global_time);
            encoder.write_frame(pixmap.data())?;
            frames_written += 1;
        }

        // Hold — render every 3 frames, repeat each 3x
        let mut local_frame = hold_start;
        while local_frame < hold_end {
            if max_frames.is_some_and(|m| frames_written >= m) { break; }
            let global_time = (start_frame + local_frame) as f64 / FPS as f64;
            let pixmap = render_frame(&mut renderer, segment, local_frame, global_time);
            let frame_data = pixmap.data();

            let repeat = hold_step.min(hold_end - local_frame);
            for _ in 0..repeat {
                if max_frames.is_some_and(|m| frames_written >= m) { break; }
                encoder.write_frame(frame_data)?;
                frames_written += 1;
            }
            local_frame += repeat;
        }

        // Fade out
        for local_frame in hold_end..duration {
            if max_frames.is_some_and(|m| frames_written >= m) { break; }
            let global_time = (start_frame + local_frame) as f64 / FPS as f64;
            let pixmap = render_frame(&mut renderer, segment, local_frame, global_time);
            encoder.write_frame(pixmap.data())?;
            frames_written += 1;
        }

        prev_end_frame = start_frame + duration;

        if max_frames.is_some_and(|m| frames_written >= m) { break; }

        // Progress
        let pct = (frames_written as f64 / total_frames as f64 * 100.0) as u32;
        eprint!(
            "\r  Rendering: {}/{}  frames ({}%)",
            frames_written, total_frames, pct
        );
    }

    eprintln!();
    encoder.finish()?;

    Ok(())
}
