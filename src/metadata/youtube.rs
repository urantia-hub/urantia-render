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

    let title = if is_foreword {
        "Foreword | The Urantia Book".to_string()
    } else {
        format!("Paper {}: {} | The Urantia Book", paper_id, manifest.paper_title)
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

    let description = [
        &intro_text,
        "",
        &format!(
            "Read along with Paper {} of The Urantia Book, narrated with AI voice.",
            paper_id
        ),
        &format!("Duration: {} minutes", minutes),
        "",
        "Chapters:",
        &chapters.join("\n"),
        "",
        "Read the full text at https://urantiahub.com",
        "API & developer tools at https://urantia.dev",
        "",
        "#UrantiaBook #Spirituality #AudioBook",
    ]
    .join("\n");

    let tags = vec![
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
- **Playlist**: The Urantia Book — Full Audio
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
