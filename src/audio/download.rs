use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

use crate::config::audio_url;
use crate::data::paper::Paper;

/// Download all audio files for a paper from the CDN.
/// Skips files that already exist (resume-safe).
pub async fn download_paper_audio(paper: &Paper, output_dir: &Path) -> Result<(usize, usize)> {
    let paper_dir = output_dir.join(&paper.paper_id);
    fs::create_dir_all(&paper_dir).await?;

    let mut downloads: Vec<(String, PathBuf)> = Vec::new();

    // Paper intro audio
    let paper_gid = format!("{}:{}.-.-", paper.part_id, paper.paper_id);
    downloads.push((paper_gid.clone(), paper_dir.join(format!("{}.mp3", paper_gid))));

    for section in &paper.sections {
        // Section intro audio (skip section 0)
        if section.section_id != "0" && section.section_title.is_some() {
            let section_gid = format!(
                "{}:{}.{}.-",
                paper.part_id, paper.paper_id, section.section_id
            );
            downloads.push((
                section_gid.clone(),
                paper_dir.join(format!("{}.mp3", section_gid)),
            ));
        }

        // Paragraph audio
        for para in &section.paragraphs {
            downloads.push((
                para.global_id.clone(),
                paper_dir.join(format!("{}.mp3", para.global_id)),
            ));
        }
    }

    let client = reqwest::Client::new();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(crate::config::DOWNLOAD_CONCURRENCY));

    let mut handles = Vec::new();
    for (global_id, dest) in downloads {
        let client = client.clone();
        let sem = semaphore.clone();

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            if dest.exists() {
                return Ok::<bool, anyhow::Error>(false); // skipped
            }

            let url = audio_url(&global_id);
            let resp = client.get(&url).send().await?;

            if !resp.status().is_success() {
                eprintln!("  Warning: failed to download {}: {}", url, resp.status());
                return Ok(false);
            }

            let bytes = resp.bytes().await?;
            fs::write(&dest, &bytes).await?;
            Ok(true)
        }));
    }

    let mut downloaded = 0usize;
    let mut skipped = 0usize;

    for handle in handles {
        match handle.await? {
            Ok(true) => downloaded += 1,
            Ok(false) => skipped += 1,
            Err(e) => eprintln!("  Download error: {}", e),
        }
    }

    Ok((downloaded, skipped))
}
