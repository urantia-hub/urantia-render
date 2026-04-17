use anyhow::{Context, Result};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::config::{FADE_FRAMES, FPS};
use crate::data::manifest::{PaperManifest, Segment};

pub const SAMPLE_RATE: u32 = 44100;

/// Resolve audio file path, checking two layouts:
///   1. Nested: {audio_dir}/{paperId}/{globalId}.mp3
///   2. Flat:   {audio_dir}/tts-1-hd-nova-{globalId}.mp3
fn resolve_audio_path(audio_dir: &Path, paper_id: &str, global_id: &str) -> Option<std::path::PathBuf> {
    // Try nested layout first
    let nested = audio_dir.join(paper_id).join(format!("{}.mp3", global_id));
    if nested.exists() {
        return Some(nested);
    }

    // Try flat layout (urantia-data-sources)
    let flat = audio_dir.join(format!("tts-1-hd-nova-{}.mp3", global_id));
    if flat.exists() {
        return Some(flat);
    }

    None
}

/// Decode an MP3 file to mono i16 PCM samples at native sample rate.
/// Returns (samples, sample_rate).
fn decode_mp3(path: &Path) -> Result<(Vec<i16>, u32)> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open audio: {}", path.display()))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    hint.with_extension("mp3");

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .context("Failed to probe MP3")?;

    let mut format = probed.format;
    let track = format.default_track().context("No audio track found")?;
    let track_id = track.id;
    let source_rate = track.codec_params.sample_rate.unwrap_or(SAMPLE_RATE);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("Failed to create decoder")?;

    let mut all_samples: Vec<i16> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(e.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder.decode(&packet)?;
        let spec = *decoded.spec();
        let num_frames = decoded.capacity();

        let mut sample_buf = SampleBuffer::<i16>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        let samples = sample_buf.samples();

        // Convert to mono if stereo
        if spec.channels.count() == 2 {
            for chunk in samples.chunks(2) {
                let mono = ((chunk[0] as i32 + chunk[1] as i32) / 2) as i16;
                all_samples.push(mono);
            }
        } else {
            all_samples.extend_from_slice(samples);
        }
    }

    Ok((all_samples, source_rate))
}

/// Build a single PCM audio buffer for an entire paper video.
/// Each audio segment is placed at its exact sample offset.
/// Silence (zeros) fills gaps naturally.
/// Returns (samples, sample_rate).
pub fn build_audio_buffer(
    manifest: &PaperManifest,
    audio_dir: &Path,
) -> Result<(Vec<i16>, u32)> {
    // Detect sample rate from first audio file.
    // Supports two layouts:
    //   Nested: {audio_dir}/{paperId}/{globalId}.mp3
    //   Flat:   {audio_dir}/tts-1-hd-nova-{globalId}.mp3  (urantia-data-sources layout)
    let paper_dir = audio_dir.join(&manifest.paper_id);
    let first_gid = format!("{}:{}.-.-", manifest.part_id, manifest.paper_id);
    let first_path = resolve_audio_path(audio_dir, &manifest.paper_id, &first_gid);
    let detected_rate = if let Some(ref p) = first_path {
        decode_mp3(p)?.1
    } else {
        SAMPLE_RATE
    };

    let total_samples =
        (manifest.total_duration_frames as f64 / FPS as f64 * detected_rate as f64) as usize;

    let mut buffer = vec![0i16; total_samples];

    for segment in &manifest.segments {
        let (global_id, start_frame) = match segment {
            Segment::Intro {
                start_frame,
                ..
            } => {
                let gid = format!("{}:{}.-.-", manifest.part_id, manifest.paper_id);
                (gid, *start_frame)
            }
            Segment::SectionCard {
                section_title,
                start_frame,
                ..
            } => {
                // Find the section ID from the next paragraph segment
                let seg_idx = manifest
                    .segments
                    .iter()
                    .position(|s| std::ptr::eq(s, segment))
                    .unwrap_or(0);
                let next_para = manifest.segments[seg_idx..]
                    .iter()
                    .find(|s| matches!(s, Segment::Paragraph { .. }));

                if let Some(Segment::Paragraph { global_id, .. }) = next_para {
                    // Parse section ID from paragraph's globalId: "partId:paperId.sectionId.paraId"
                    let parts: Vec<&str> = global_id.split(':').collect();
                    if parts.len() == 2 {
                        let sub: Vec<&str> = parts[1].split('.').collect();
                        if sub.len() >= 2 {
                            let section_gid = format!("{}:{}.{}.-", parts[0], sub[0], sub[1]);
                            // Delay section audio by FADE_FRAMES so speech begins
                            // once the card has fully faded in, not during the
                            // fade-in phase. Makes the section title feel like a
                            // deliberate announcement at peak visibility.
                            (section_gid, *start_frame + FADE_FRAMES)
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            Segment::Paragraph {
                global_id,
                start_frame,
                ..
            } => (global_id.clone(), *start_frame),
            Segment::Outro { .. } => continue, // no audio for outro
        };

        let audio_path = match resolve_audio_path(audio_dir, &manifest.paper_id, &global_id) {
            Some(p) => p,
            None => {
                eprintln!("  Warning: audio not found for {}", global_id);
                continue;
            }
        };

        let samples = match decode_mp3(&audio_path) {
            Ok((s, _rate)) => s,
            Err(e) => {
                eprintln!("  Warning: failed to decode {}: {}", audio_path.display(), e);
                continue;
            }
        };

        // Place samples at the correct offset
        let sample_offset =
            (start_frame as f64 / FPS as f64 * detected_rate as f64) as usize;

        let copy_len = samples.len().min(buffer.len().saturating_sub(sample_offset));
        buffer[sample_offset..sample_offset + copy_len]
            .copy_from_slice(&samples[..copy_len]);
    }

    Ok((buffer, detected_rate))
}

/// Write PCM buffer as a WAV file for ffmpeg input.
pub fn write_wav(samples: &[i16], sample_rate: u32, path: &Path) -> Result<()> {
    let data_size = (samples.len() * 2) as u32;
    let file_size = 36 + data_size;

    let mut buf: Vec<u8> = Vec::with_capacity(file_size as usize + 8);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    buf.extend_from_slice(&1u16.to_le_bytes()); // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    buf.extend_from_slice(&2u16.to_le_bytes()); // block align
    buf.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    for &sample in samples {
        buf.extend_from_slice(&sample.to_le_bytes());
    }

    std::fs::write(path, &buf)?;
    Ok(())
}
