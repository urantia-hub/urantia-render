use anyhow::{Context, Result};
use s3::creds::Credentials;
use s3::region::Region;
use s3::Bucket;
use std::path::Path;

use crate::config::{R2_BUCKET, video_filename};

fn get_bucket() -> Result<Box<Bucket>> {
    let endpoint = std::env::var("R2_ENDPOINT").context("R2_ENDPOINT not set")?;
    let access_key = std::env::var("R2_ACCESS_KEY_ID").context("R2_ACCESS_KEY_ID not set")?;
    let secret_key =
        std::env::var("R2_SECRET_ACCESS_KEY").context("R2_SECRET_ACCESS_KEY not set")?;

    let region = Region::Custom {
        region: "auto".to_string(),
        endpoint,
    };

    let credentials = Credentials::new(Some(&access_key), Some(&secret_key), None, None, None)
        .context("Failed to create R2 credentials")?;

    let bucket = Bucket::new(R2_BUCKET, region, credentials)
        .context("Failed to create R2 bucket")?
        .with_path_style();

    Ok(bucket)
}

/// Upload a video file to R2. Returns the public URL.
/// Skips if the object already exists (unless force=true).
pub async fn upload_video(
    paper_id: &str,
    videos_dir: &Path,
    force: bool,
    dry_run: bool,
) -> Result<Option<String>> {
    let file_name = video_filename(paper_id);
    let local_path = videos_dir.join(&file_name);

    if !local_path.exists() {
        return Ok(None);
    }

    let size_mb = std::fs::metadata(&local_path)?.len() as f64 / 1024.0 / 1024.0;

    if dry_run {
        println!("  [DRY RUN] Would upload: {} ({:.1} MB)", file_name, size_mb);
        return Ok(Some(file_name));
    }

    let bucket = get_bucket()?;

    // Check if already exists
    if !force {
        match bucket.head_object(&file_name).await {
            Ok(_) => {
                println!("  Skipping {}: already uploaded", file_name);
                return Ok(Some(file_name));
            }
            Err(_) => {} // doesn't exist, proceed
        }
    }

    println!("  Uploading {} ({:.1} MB)...", file_name, size_mb);

    let content = std::fs::read(&local_path)?;
    bucket
        .put_object_with_content_type(&file_name, &content, "video/mp4")
        .await
        .context("Failed to upload to R2")?;

    let url = format!("https://video.urantia.dev/{}", file_name);
    println!("  Done: {}", url);

    Ok(Some(file_name))
}

/// Upload a thumbnail PNG to R2.
pub async fn upload_thumbnail(
    paper_id: &str,
    thumbnails_dir: &Path,
    force: bool,
    dry_run: bool,
) -> Result<Option<String>> {
    let file_name = format!("thumbnail-{}.png", paper_id);
    let local_path = thumbnails_dir.join(&file_name);

    if !local_path.exists() {
        return Ok(None);
    }

    let size_kb = std::fs::metadata(&local_path)?.len() as f64 / 1024.0;

    if dry_run {
        println!("  [DRY RUN] Would upload: {} ({:.0} KB)", file_name, size_kb);
        return Ok(Some(file_name));
    }

    let bucket = get_bucket()?;

    if !force {
        match bucket.head_object(&file_name).await {
            Ok(_) => {
                return Ok(Some(file_name));
            }
            Err(_) => {}
        }
    }

    let content = std::fs::read(&local_path)?;
    bucket
        .put_object_with_content_type(&file_name, &content, "image/png")
        .await
        .context("Failed to upload thumbnail to R2")?;

    Ok(Some(file_name))
}
