use crate::config::*;
use crate::data::audio_manifest::AudioManifest;
use crate::data::paper::Paper;
use crate::data::text_chunker::{chunk_text, TextChunk};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Segment {
    Intro {
        paper_title: String,
        paper_id: String,
        start_frame: u32,
        duration_frames: u32,
    },
    SectionCard {
        section_title: String,
        start_frame: u32,
        duration_frames: u32,
    },
    Paragraph {
        global_id: String,
        standard_reference_id: String,
        text: String,
        section_title: Option<String>,
        audio_duration_sec: f64,
        start_frame: u32,
        duration_frames: u32,
        text_chunks: Vec<TextChunk>,
    },
    Outro {
        start_frame: u32,
        duration_frames: u32,
        #[serde(default)]
        tagline: Option<String>,
    },
}

impl Segment {
    pub fn start_frame(&self) -> u32 {
        match self {
            Segment::Intro { start_frame, .. } => *start_frame,
            Segment::SectionCard { start_frame, .. } => *start_frame,
            Segment::Paragraph { start_frame, .. } => *start_frame,
            Segment::Outro { start_frame, .. } => *start_frame,
        }
    }

    pub fn duration_frames(&self) -> u32 {
        match self {
            Segment::Intro { duration_frames, .. } => *duration_frames,
            Segment::SectionCard { duration_frames, .. } => *duration_frames,
            Segment::Paragraph { duration_frames, .. } => *duration_frames,
            Segment::Outro { duration_frames, .. } => *duration_frames,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperManifest {
    pub paper_id: String,
    pub paper_title: String,
    pub part_id: String,
    pub fps: u32,
    pub segments: Vec<Segment>,
    pub total_duration_frames: u32,
    pub total_duration_sec: u32,
}

pub fn build_manifest(paper: &Paper, audio_manifest: &AudioManifest) -> PaperManifest {
    let mut segments = Vec::new();
    let mut current_frame = 0u32;

    // Intro card
    let paper_global_id = format!("{}:{}.-.-", paper.part_id, paper.paper_id);
    let intro_audio_dur = audio_manifest.get_duration(&paper_global_id);
    let intro_frames = if let Some(dur) = intro_audio_dur {
        (dur * FPS as f64).ceil() as u32 + (INTRO_PADDING_SEC * FPS as f64) as u32
    } else {
        5 * FPS // fallback 5 seconds
    };

    segments.push(Segment::Intro {
        paper_title: paper.paper_title.clone(),
        paper_id: paper.paper_id.clone(),
        start_frame: current_frame,
        duration_frames: intro_frames,
    });
    current_frame += intro_frames;

    // Sections
    for section in &paper.sections {
        // Section title card (skip section 0)
        if section.section_title.is_some() && section.section_id != "0" {
            let section_global_id = format!(
                "{}:{}.{}.-",
                paper.part_id, paper.paper_id, section.section_id
            );
            let section_audio_dur = audio_manifest.get_duration(&section_global_id);
            let section_frames = if let Some(dur) = section_audio_dur {
                (dur * FPS as f64).ceil() as u32 + (SECTION_CARD_PADDING_SEC * FPS as f64) as u32
            } else {
                SECTION_CARD_FRAMES
            };

            segments.push(Segment::SectionCard {
                section_title: section.section_title.clone().unwrap_or_default(),
                start_frame: current_frame,
                duration_frames: section_frames,
            });
            current_frame += section_frames;
        }

        // Paragraphs
        for para in &section.paragraphs {
            let duration = match audio_manifest.get_duration(&para.global_id) {
                Some(d) => d,
                None => {
                    eprintln!("Warning: no audio for {}, skipping", para.global_id);
                    continue;
                }
            };

            let duration_frames = (duration * FPS as f64).ceil() as u32;
            let text_chunks = chunk_text(&para.text, duration, duration_frames);

            segments.push(Segment::Paragraph {
                global_id: para.global_id.clone(),
                standard_reference_id: para.standard_reference_id.clone(),
                text: para.text.clone(),
                section_title: para.section_title.clone(),
                audio_duration_sec: duration,
                start_frame: current_frame,
                duration_frames,
                text_chunks,
            });
            current_frame += duration_frames;
        }
    }

    // Outro
    segments.push(Segment::Outro {
        start_frame: current_frame,
        duration_frames: OUTRO_FRAMES,
        tagline: None,
    });
    current_frame += OUTRO_FRAMES;

    PaperManifest {
        paper_id: paper.paper_id.clone(),
        paper_title: paper.paper_title.clone(),
        part_id: paper.part_id.clone(),
        fps: FPS,
        segments,
        total_duration_frames: current_frame,
        total_duration_sec: current_frame / FPS,
    }
}
