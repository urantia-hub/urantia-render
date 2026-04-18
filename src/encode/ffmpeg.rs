use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::config::{FPS, HEIGHT, WIDTH};

/// An ffmpeg subprocess that accepts raw RGBA frames via stdin
/// and produces an H.264 MP4 with audio.
pub struct FfmpegEncoder {
    child: Child,
    frame_size: usize,
}

impl FfmpegEncoder {
    /// Start ffmpeg with raw video input (pipe) + WAV audio input.
    ///
    /// Encoder selection:
    /// - macOS: `h264_videotoolbox` (Apple Media Engine, near real-time 4K).
    /// - Linux/other: `libx264 -preset fast` (software, used on AWS spot boxes).
    ///   Override with `URANTIA_RENDER_ENCODER=libx264|videotoolbox|nvenc` if
    ///   auto-detection picks the wrong one (e.g. Linux host with an NVIDIA GPU).
    /// - `URANTIA_RENDER_THREADS=N` caps per-ffmpeg thread count so that
    ///   concurrent pipelines on a multi-core box don't oversubscribe.
    pub fn new(output_path: &Path, audio_wav_path: &Path) -> Result<Self> {
        let encoder = encoder_choice();
        let encoder_args = encoder_args(&encoder);

        let size_str = format!("{}x{}", WIDTH, HEIGHT);
        let fps_str = FPS.to_string();
        let audio_str = audio_wav_path.to_string_lossy().into_owned();
        let output_str = output_path.to_string_lossy().into_owned();
        let threads_env = std::env::var("URANTIA_RENDER_THREADS").ok();

        let mut args: Vec<&str> = vec![
            "-y",
            "-f", "rawvideo",
            "-pix_fmt", "rgba",
            "-s", &size_str,
            "-r", &fps_str,
            "-i", "pipe:0",
            "-i", &audio_str,
        ];
        args.extend(encoder_args.iter().map(|s| s.as_str()));
        if let Some(t) = &threads_env {
            args.extend_from_slice(&["-threads", t.as_str()]);
        }
        args.extend_from_slice(&[
            "-c:a", "aac",
            "-b:a", "128k",
            "-movflags", "+faststart",
            "-shortest",
            &output_str,
        ]);

        let child = Command::new("ffmpeg")
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context(
                "Failed to start ffmpeg. Install it (macOS: `brew install ffmpeg`; \
                 Debian/Ubuntu: `apt install ffmpeg`).",
            )?;

        Ok(Self {
            child,
            frame_size: (WIDTH * HEIGHT * 4) as usize,
        })
    }

    /// Write a single RGBA frame to ffmpeg's stdin.
    pub fn write_frame(&mut self, rgba_data: &[u8]) -> Result<()> {
        debug_assert_eq!(rgba_data.len(), self.frame_size);

        let stdin = self
            .child
            .stdin
            .as_mut()
            .context("ffmpeg stdin not available")?;

        stdin.write_all(rgba_data).context("Failed to write frame to ffmpeg")?;

        Ok(())
    }

    /// Close stdin and wait for ffmpeg to finish encoding.
    pub fn finish(mut self) -> Result<()> {
        // Close stdin to signal end of input
        drop(self.child.stdin.take());

        let output = self.child.wait_with_output().context("ffmpeg process failed")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("ffmpeg exited with {}: {}", output.status, stderr);
        }

        Ok(())
    }
}

fn encoder_choice() -> String {
    if let Ok(e) = std::env::var("URANTIA_RENDER_ENCODER") {
        return e;
    }
    if cfg!(target_os = "macos") {
        "videotoolbox".to_string()
    } else {
        "libx264".to_string()
    }
}

/// Per-encoder ffmpeg arguments. Quality tuned for "good enough for YouTube
/// re-encode"  — YouTube re-encodes every upload to VP9/AV1 anyway, so we
/// don't need archival-grade output, just clean edges on gray-text-on-dark.
fn encoder_args(encoder: &str) -> Vec<String> {
    match encoder {
        "videotoolbox" | "h264_videotoolbox" => vec![
            "-c:v".into(), "h264_videotoolbox".into(),
            "-q:v".into(), "65".into(),
            "-profile:v".into(), "high".into(),
            "-pix_fmt".into(), "yuv420p".into(),
        ],
        "nvenc" | "h264_nvenc" => vec![
            "-c:v".into(), "h264_nvenc".into(),
            "-preset".into(), "p4".into(),
            "-tune".into(), "hq".into(),
            "-rc".into(), "vbr".into(),
            "-cq".into(), "22".into(),
            "-b:v".into(), "0".into(),
            "-pix_fmt".into(), "yuv420p".into(),
        ],
        // libx264 default. `-preset fast` trades a bit of compression for
        // ~3× encode speed — perfect for our low-entropy text-on-dark content
        // where the codec isn't working hard anyway.
        _ => vec![
            "-c:v".into(), "libx264".into(),
            "-preset".into(), "fast".into(),
            "-crf".into(), "20".into(),
            "-profile:v".into(), "high".into(),
            "-pix_fmt".into(), "yuv420p".into(),
        ],
    }
}
