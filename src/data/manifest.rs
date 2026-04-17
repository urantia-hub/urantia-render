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
    // Silence between intro and the next segment.
    current_frame += GAP_AFTER_INTRO_FRAMES;

    // Sections
    for section in &paper.sections {
        let has_section_card = section.section_title.is_some() && section.section_id != "0";

        if has_section_card {
            // Silence before the section card when transitioning from a paragraph.
            // (First section: GAP_AFTER_INTRO is already applied — don't double up.)
            let prev_was_paragraph = matches!(
                segments.last(),
                Some(Segment::Paragraph { .. })
            );
            if prev_was_paragraph {
                current_frame += GAP_BEFORE_SECTION_FRAMES;
            }

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
            // Silence after the section card before its first paragraph.
            current_frame += GAP_AFTER_SECTION_FRAMES;
        }

        // Paragraphs
        let mut first_paragraph_in_section = true;
        for para in &section.paragraphs {
            let duration = match audio_manifest.get_duration(&para.global_id) {
                Some(d) => d,
                None => {
                    eprintln!("Warning: no audio for {}, skipping", para.global_id);
                    continue;
                }
            };

            // Silence between paragraphs within the same section.
            // First paragraph after a card (intro or section) already has its
            // GAP_AFTER_* applied — skip.
            if !first_paragraph_in_section {
                current_frame += GAP_BETWEEN_PARAGRAPHS_FRAMES;
            }
            first_paragraph_in_section = false;

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

    // Outro (no preceding gap)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::audio_manifest::AudioManifest;
    use crate::data::paper::{Paper, Paragraph, Section};
    use std::collections::HashMap;

    fn fixture_one_section_two_paragraphs() -> (Paper, AudioManifest) {
        // Paper 1, part 1. One paper intro (2.0s), one section with title,
        // 1 section intro (1.5s), 2 paragraphs (3.0s each).
        let paper = Paper {
            paper_id: "1".into(),
            part_id: "1".into(),
            paper_title: "Test Paper".into(),
            sections: vec![Section {
                section_id: "1".into(),
                section_title: Some("Test Section".into()),
                paragraphs: vec![
                    Paragraph {
                        global_id: "1:1.1.1".into(),
                        standard_reference_id: "1.1.1".into(),
                        text: "First paragraph.".into(),
                        section_title: Some("Test Section".into()),
                        section_id: "1".into(),
                    },
                    Paragraph {
                        global_id: "1:1.1.2".into(),
                        standard_reference_id: "1.1.2".into(),
                        text: "Second paragraph.".into(),
                        section_title: Some("Test Section".into()),
                        section_id: "1".into(),
                    },
                ],
            }],
        };

        let mut durations = HashMap::new();
        durations.insert("1:1.-.-".into(), 2.0);
        durations.insert("1:1.1.-".into(), 1.5);
        durations.insert("1:1.1.1".into(), 3.0);
        durations.insert("1:1.1.2".into(), 3.0);
        let audio = AudioManifest::from_durations_for_test(durations);

        (paper, audio)
    }

    #[test]
    fn gap_after_intro_is_inserted_before_section_card() {
        let (paper, audio) = fixture_one_section_two_paragraphs();
        let m = build_manifest(&paper, &audio);

        // Intro audio 2.0s * 30fps = 60 frames + INTRO_PADDING_SEC 1.0s * 30 = 30 frames.
        let expected_intro_frames = (2.0 * FPS as f64).ceil() as u32
            + (INTRO_PADDING_SEC * FPS as f64) as u32;

        let section_start = m.segments.iter().find_map(|s| match s {
            Segment::SectionCard { start_frame, .. } => Some(*start_frame),
            _ => None,
        }).expect("section card missing");

        assert_eq!(
            section_start,
            expected_intro_frames + GAP_AFTER_INTRO_FRAMES
        );
    }

    #[test]
    fn gap_between_paragraphs_is_inserted() {
        let (paper, audio) = fixture_one_section_two_paragraphs();
        let m = build_manifest(&paper, &audio);

        let paragraph_starts: Vec<u32> = m.segments.iter().filter_map(|s| match s {
            Segment::Paragraph { start_frame, .. } => Some(*start_frame),
            _ => None,
        }).collect();
        assert_eq!(paragraph_starts.len(), 2, "should have 2 paragraphs");

        let first_duration_frames = (3.0 * FPS as f64).ceil() as u32;
        assert_eq!(
            paragraph_starts[1],
            paragraph_starts[0] + first_duration_frames + GAP_BETWEEN_PARAGRAPHS_FRAMES
        );
    }

    #[test]
    fn gap_after_section_card_is_inserted_before_first_paragraph() {
        let (paper, audio) = fixture_one_section_two_paragraphs();
        let m = build_manifest(&paper, &audio);

        let section_end = m.segments.iter().find_map(|s| match s {
            Segment::SectionCard { start_frame, duration_frames, .. } => {
                Some(*start_frame + *duration_frames)
            }
            _ => None,
        }).expect("section card missing");

        let first_paragraph_start = m.segments.iter().find_map(|s| match s {
            Segment::Paragraph { start_frame, .. } => Some(*start_frame),
            _ => None,
        }).expect("first paragraph missing");

        assert_eq!(first_paragraph_start, section_end + GAP_AFTER_SECTION_FRAMES);
    }

    #[test]
    fn gap_before_section_card_is_inserted_when_transitioning_from_paragraph() {
        // Paper with two sections: ensure the gap between end-of-section-1-paragraph
        // and start-of-section-2-card is GAP_BEFORE_SECTION.
        let paper = Paper {
            paper_id: "1".into(),
            part_id: "1".into(),
            paper_title: "Test Paper".into(),
            sections: vec![
                Section {
                    section_id: "1".into(),
                    section_title: Some("Section One".into()),
                    paragraphs: vec![Paragraph {
                        global_id: "1:1.1.1".into(),
                        standard_reference_id: "1.1.1".into(),
                        text: "P1.".into(),
                        section_title: Some("Section One".into()),
                        section_id: "1".into(),
                    }],
                },
                Section {
                    section_id: "2".into(),
                    section_title: Some("Section Two".into()),
                    paragraphs: vec![Paragraph {
                        global_id: "1:1.2.1".into(),
                        standard_reference_id: "1.2.1".into(),
                        text: "P2.".into(),
                        section_title: Some("Section Two".into()),
                        section_id: "2".into(),
                    }],
                },
            ],
        };
        let mut durations = HashMap::new();
        durations.insert("1:1.-.-".into(), 2.0);
        durations.insert("1:1.1.-".into(), 1.5);
        durations.insert("1:1.1.1".into(), 3.0);
        durations.insert("1:1.2.-".into(), 1.5);
        durations.insert("1:1.2.1".into(), 3.0);
        let audio = AudioManifest::from_durations_for_test(durations);

        let m = build_manifest(&paper, &audio);

        let section_cards: Vec<u32> = m.segments.iter().filter_map(|s| match s {
            Segment::SectionCard { start_frame, .. } => Some(*start_frame),
            _ => None,
        }).collect();
        assert_eq!(section_cards.len(), 2);

        let first_paragraph_end = m.segments.iter().find_map(|s| match s {
            Segment::Paragraph { start_frame, duration_frames, global_id, .. }
                if global_id == "1:1.1.1" => Some(*start_frame + *duration_frames),
            _ => None,
        }).expect("paragraph 1:1.1.1 missing");

        assert_eq!(section_cards[1], first_paragraph_end + GAP_BEFORE_SECTION_FRAMES);
    }
}
