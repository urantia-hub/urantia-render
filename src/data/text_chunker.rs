use crate::config::{MIN_AUDIO_DURATION_FOR_SPLIT, MIN_CHARS_FOR_SPLIT, MIN_CHUNK_CHARS};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextChunk {
    pub text: String,
    pub start_frame: u32,
    pub duration_frames: u32,
}

/// Duration-aware text chunking. Splits long paragraphs at sentence boundaries.
pub fn chunk_text(text: &str, audio_duration_sec: f64, total_duration_frames: u32) -> Vec<TextChunk> {
    let needs_split = audio_duration_sec > MIN_AUDIO_DURATION_FOR_SPLIT
        && text.len() > MIN_CHARS_FOR_SPLIT;

    if !needs_split {
        return vec![TextChunk {
            text: text.to_string(),
            start_frame: 0,
            duration_frames: total_duration_frames,
        }];
    }

    let sentences = split_into_sentences(text);
    let target_chunk_chars = (text.len() as f64 / (text.len() as f64 / 400.0).ceil()) as usize;

    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();

    for sentence in &sentences {
        if !current.is_empty()
            && current.len() + sentence.len() > target_chunk_chars
            && current.len() >= MIN_CHUNK_CHARS
        {
            chunks.push(current.trim().to_string());
            current = sentence.to_string();
        } else {
            current.push_str(sentence);
        }
    }

    if !current.trim().is_empty() {
        if chunks.len() > 0 && current.trim().len() < MIN_CHUNK_CHARS {
            if let Some(last) = chunks.last_mut() {
                last.push(' ');
                last.push_str(current.trim());
            }
        } else {
            chunks.push(current.trim().to_string());
        }
    }

    if chunks.len() <= 1 {
        return vec![TextChunk {
            text: text.to_string(),
            start_frame: 0,
            duration_frames: total_duration_frames,
        }];
    }

    // Distribute frames proportionally by character count
    let total_chars: usize = chunks.iter().map(|c| c.len()).sum();
    let mut result = Vec::new();
    let mut frame_offset = 0u32;

    for (i, chunk) in chunks.iter().enumerate() {
        let proportion = chunk.len() as f64 / total_chars as f64;
        let frames = if i == chunks.len() - 1 {
            total_duration_frames - frame_offset
        } else {
            (proportion * total_duration_frames as f64).round() as u32
        };

        result.push(TextChunk {
            text: chunk.clone(),
            start_frame: frame_offset,
            duration_frames: frames,
        });
        frame_offset += frames;
    }

    result
}

fn split_into_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        current.push(chars[i]);

        if (chars[i] == '.' || chars[i] == '?' || chars[i] == '!')
            && i + 1 < chars.len()
            && chars[i + 1] == ' '
        {
            current.push(' ');
            sentences.push(current.clone());
            current.clear();
            i += 2; // skip the space
            continue;
        }

        i += 1;
    }

    if !current.is_empty() {
        sentences.push(current);
    }

    sentences
}
