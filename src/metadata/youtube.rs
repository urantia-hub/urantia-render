use anyhow::Result;
use serde::Serialize;
use std::path::Path;

use crate::config;
use crate::data::manifest::{PaperManifest, Segment};

#[derive(Debug, Serialize)]
pub struct VideoMetadata {
    pub paper_id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub duration_sec: u32,
    pub file_name: String,
}

fn format_timestamp(total_seconds: u32) -> String {
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

pub fn generate_metadata(manifest: &PaperManifest) -> VideoMetadata {
    let paper_id = &manifest.paper_id;
    let is_foreword = paper_id == "0";

    // Title format:
    //   Foreword -> "Foreword | The Urantia Papers"
    //   Paper N  -> "{title} — Paper N | The Urantia Papers"
    //
    // Note: paper_title was normalized at data load (Paper::from_json), so titles
    // like "Supreme and Ultimate — Time and Space" already have spaced em-dashes.
    let title = if is_foreword {
        "Foreword | The Urantia Papers".to_string()
    } else {
        format!(
            "{} — Paper {} | The Urantia Papers",
            manifest.paper_title, paper_id
        )
    };

    // Chapter timestamps from section cards
    let mut chapters = vec!["0:00 Introduction".to_string()];
    for segment in &manifest.segments {
        if let Segment::SectionCard {
            section_title,
            start_frame,
            ..
        } = segment
        {
            let timestamp = format_timestamp(start_frame / manifest.fps);
            chapters.push(format!("{} {}", timestamp, section_title));
        }
    }

    // First paragraph text for description
    let intro_text = manifest
        .segments
        .iter()
        .find_map(|s| {
            if let Segment::Paragraph { text, .. } = s {
                Some(if text.len() > 300 {
                    format!("{}...", &text[..300])
                } else {
                    text.clone()
                })
            } else {
                None
            }
        })
        .unwrap_or_default();

    let minutes = manifest.total_duration_sec / 60;

    let read_along_line = if is_foreword {
        "Read along with the Foreword of The Urantia Papers, narrated with AI voice.".to_string()
    } else {
        format!(
            "Read along with Paper {} of The Urantia Papers, narrated with AI voice.",
            paper_id
        )
    };

    let description = [
        &intro_text,
        "",
        &read_along_line,
        &format!("Duration: {} minutes", minutes),
        "",
        "Chapters:",
        &chapters.join("\n"),
        "",
        "Read the full text at https://urantiahub.com",
        "API & developer tools at https://urantia.dev",
        "",
        "#UrantiaPapers #UrantiaBook #Spirituality #AudioBook",
    ]
    .join("\n");

    let tags = vec![
        "Urantia Papers".to_string(),
        "Urantia Book".to_string(),
        "Urantia".to_string(),
        manifest.paper_title.clone(),
        "audiobook".to_string(),
        "spirituality".to_string(),
        "philosophy".to_string(),
        "theology".to_string(),
        "UrantiaHub".to_string(),
    ];

    let file_name = config::video_filename(paper_id);

    VideoMetadata {
        paper_id: paper_id.clone(),
        title,
        description,
        tags,
        duration_sec: manifest.total_duration_sec,
        file_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::manifest::PaperManifest;

    fn manifest_for(paper_id: &str, paper_title: &str) -> PaperManifest {
        PaperManifest {
            paper_id: paper_id.to_string(),
            paper_title: paper_title.to_string(),
            part_id: "1".to_string(),
            fps: 30,
            segments: Vec::new(),
            total_duration_frames: 9000,
            total_duration_sec: 300,
        }
    }

    #[test]
    fn foreword_title_format() {
        let m = manifest_for("0", "Foreword");
        let meta = generate_metadata(&m);
        assert_eq!(meta.title, "Foreword | The Urantia Papers");
    }

    #[test]
    fn regular_paper_title_format() {
        let m = manifest_for("1", "The Universal Father");
        let meta = generate_metadata(&m);
        assert_eq!(
            meta.title,
            "The Universal Father — Paper 1 | The Urantia Papers"
        );
    }

    #[test]
    fn paper_with_em_dash_title_format() {
        let m = manifest_for("118", "Supreme and Ultimate — Time and Space");
        let meta = generate_metadata(&m);
        assert_eq!(
            meta.title,
            "Supreme and Ultimate — Time and Space — Paper 118 | The Urantia Papers"
        );
    }
}

pub fn generate_and_write(
    manifest: &PaperManifest,
    output_dir: &Path,
) -> Result<VideoMetadata> {
    let metadata = generate_metadata(manifest);

    // Write JSON
    let json = serde_json::to_string_pretty(&metadata)?;
    std::fs::write(
        output_dir.join(format!("{}.json", metadata.paper_id)),
        &json,
    )?;

    // Write copy-paste upload sheet
    let sheet = format!(
        r#"# Paper {} — YouTube Upload Sheet

## Title (copy)
```
{}
```

## Description (copy)
```
{}
```

## Tags (copy)
```
{}
```

## Settings
- **Category**: Education
- **Audience**: Not made for kids
- **Playlist**: The Urantia Papers — Full Audio
- **Visibility**: Public

## Files
- **Video**: `{}`
- **Thumbnail**: `thumbnail-{}.png`
"#,
        metadata.paper_id,
        metadata.title,
        metadata.description,
        metadata.tags.join(", "),
        metadata.file_name,
        metadata.paper_id,
    );

    let sheets_dir = output_dir.join("sheets");
    std::fs::create_dir_all(&sheets_dir)?;
    std::fs::write(
        sheets_dir.join(format!("paper-{}.md", metadata.paper_id)),
        &sheet,
    )?;

    Ok(metadata)
}
