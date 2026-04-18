use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::config;
use crate::data::manifest::{PaperManifest, Segment};
use crate::text_util::normalize_title;

/// A single entity from api.urantia.dev's topEntities aggregate.
#[derive(Debug, Clone, Deserialize)]
pub struct TopEntity {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub count: u32,
}

/// Fetch the top entities for a paper from api.urantia.dev.
/// Returns an empty Vec on any error (metadata is best-effort).
pub async fn fetch_top_entities(paper_id: &str) -> Vec<TopEntity> {
    #[derive(Deserialize)]
    struct Response {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        paper: Paper,
    }
    #[derive(Deserialize)]
    struct Paper {
        #[serde(rename = "topEntities", default)]
        top_entities: Vec<TopEntity>,
    }

    let url = format!(
        "https://api.urantia.dev/papers/{}?include=topEntities",
        paper_id
    );
    match reqwest::get(&url).await {
        Ok(res) if res.status().is_success() => match res.json::<Response>().await {
            Ok(parsed) => parsed.data.paper.top_entities,
            Err(e) => {
                eprintln!("  Warning: failed to parse topEntities for paper {}: {}", paper_id, e);
                Vec::new()
            }
        },
        Ok(res) => {
            eprintln!(
                "  Warning: topEntities fetch for paper {} returned {}",
                paper_id,
                res.status()
            );
            Vec::new()
        }
        Err(e) => {
            eprintln!("  Warning: topEntities fetch for paper {} failed: {}", paper_id, e);
            Vec::new()
        }
    }
}

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

pub fn generate_metadata(
    manifest: &PaperManifest,
    top_entities: &[TopEntity],
) -> VideoMetadata {
    let paper_id = &manifest.paper_id;
    let is_foreword = paper_id == "0";

    // Manifests cached before the normalization change may still hold unspaced
    // em-dashes; normalize defensively. normalize_title is idempotent.
    let paper_title = normalize_title(&manifest.paper_title);

    let title = if is_foreword {
        "Foreword | The Urantia Papers".to_string()
    } else {
        format!(
            "{} — Paper {} | The Urantia Papers",
            paper_title, paper_id
        )
    };

    let mut chapters = vec!["0:00 Introduction".to_string()];
    for segment in &manifest.segments {
        if let Segment::SectionCard {
            section_title,
            start_frame,
            ..
        } = segment
        {
            let timestamp = format_timestamp(start_frame / manifest.fps);
            chapters.push(format!("{} {}", timestamp, normalize_title(section_title)));
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

    // Build a "Key topics" line from the top 6 entities — real names of
    // beings / places / concepts the paper discusses, sourced from the
    // entity knowledge graph at api.urantia.dev.
    let topics_line = if top_entities.is_empty() {
        None
    } else {
        let names: Vec<String> = top_entities
            .iter()
            .take(6)
            .map(|e| e.name.clone())
            .collect();
        Some(format!("Key topics: {}", names.join(", ")))
    };

    // Assemble description, inserting the topics line between the read-along
    // blurb and the chapter list when available.
    let mut description_lines: Vec<String> = Vec::new();
    description_lines.push(intro_text.clone());
    description_lines.push(String::new());
    description_lines.push(read_along_line.clone());
    description_lines.push(format!("Duration: {} minutes", minutes));
    if let Some(line) = &topics_line {
        description_lines.push(String::new());
        description_lines.push(line.clone());
    }
    description_lines.push(String::new());
    description_lines.push("Chapters:".to_string());
    description_lines.push(chapters.join("\n"));
    description_lines.push(String::new());
    description_lines.push("Read the full text at https://urantiahub.com".to_string());
    description_lines.push("API & developer tools at https://urantia.dev".to_string());
    description_lines.push(String::new());
    description_lines.push("#UrantiaPapers #UrantiaBook #Spirituality #AudioBook".to_string());
    let description = description_lines.join("\n");

    // Tags in SEO priority order:
    //   1. Top entities (most specific, least competition, best discovery)
    //   2. Paper title (matches the YouTube title — boosts relevancy signal)
    //   3. Brand terms (Urantia Papers → Urantia Book → Urantia → UrantiaHub)
    //   4. Generic categories last (audiobook, spirituality, …) — these get
    //      buried regardless and are the first to drop when we hit the budget.
    //
    // YouTube caps total tag character count at 500. We assemble in priority
    // order and stop adding once the budget is hit — generic tags fall off
    // first, which is what we want.
    // YouTube's tag UI splits on comma, so any tag containing a comma gets
    // silently broken up (and often mangled in Studio's display). Strip commas
    // from entity names before using them as tags. Pattern: "A, B" → "A B".
    fn sanitize_tag(name: &str) -> String {
        name.replace(", ", " ").replace(',', "")
    }

    let priority_tags: Vec<String> = {
        let mut t: Vec<String> = top_entities
            .iter()
            .take(10)
            .map(|e| sanitize_tag(&e.name))
            .collect();
        // Paper title right after entities (avoid duplicating if already listed).
        // Sanitize in case the title has a comma (e.g. "Fetishes, Charms, and Magic").
        let title_tag = sanitize_tag(&paper_title);
        if !t.iter().any(|x| x.eq_ignore_ascii_case(&title_tag)) {
            t.push(title_tag);
        }
        // Brand terms.
        t.push("Urantia Papers".to_string());
        t.push("Urantia Book".to_string());
        t.push("Urantia".to_string());
        t.push("UrantiaHub".to_string());
        // Generics last.
        t.push("audiobook".to_string());
        t.push("spirituality".to_string());
        t.push("philosophy".to_string());
        t.push("theology".to_string());
        t
    };
    let mut tags: Vec<String> = Vec::new();
    let mut budget: i32 = 500;
    for tag in priority_tags {
        let cost = tag.len() as i32 + 2;
        if cost > budget {
            break;
        }
        budget -= cost;
        tags.push(tag);
    }

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
        let meta = generate_metadata(&m, &[]);
        assert_eq!(meta.title, "Foreword | The Urantia Papers");
    }

    #[test]
    fn regular_paper_title_format() {
        let m = manifest_for("1", "The Universal Father");
        let meta = generate_metadata(&m, &[]);
        assert_eq!(
            meta.title,
            "The Universal Father — Paper 1 | The Urantia Papers"
        );
    }

    #[test]
    fn paper_with_em_dash_title_format() {
        let m = manifest_for("118", "Supreme and Ultimate — Time and Space");
        let meta = generate_metadata(&m, &[]);
        assert_eq!(
            meta.title,
            "Supreme and Ultimate — Time and Space — Paper 118 | The Urantia Papers"
        );
    }
}

pub async fn generate_and_write(
    manifest: &PaperManifest,
    output_dir: &Path,
) -> Result<VideoMetadata> {
    let top_entities = fetch_top_entities(&manifest.paper_id).await;
    let metadata = generate_metadata(manifest, &top_entities);

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
