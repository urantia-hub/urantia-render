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
    pub fn new(output_path: &Path, audio_wav_path: &Path) -> Result<Self> {
        let child = Command::new("ffmpeg")
            .args([
                "-y", // overwrite
                // Video input: raw RGBA from stdin
                "-f", "rawvideo",
                "-pix_fmt", "rgba",
                "-s", &format!("{}x{}", WIDTH, HEIGHT),
                "-r", &FPS.to_string(),
                "-i", "pipe:0",
                // Audio input: WAV file
                "-i", &audio_wav_path.to_string_lossy(),
                // Video codec
                "-c:v", "libx264",
                "-preset", "medium",
                "-crf", "20",
                "-pix_fmt", "yuv420p",
                // Audio codec
                "-c:a", "aac",
                "-b:a", "128k",
                // Optimization
                "-movflags", "+faststart",
                "-shortest",
                // Output
                &output_path.to_string_lossy(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to start ffmpeg. Is it installed? (brew install ffmpeg)")?;

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
