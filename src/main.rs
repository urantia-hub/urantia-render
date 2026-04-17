mod config;
mod data;
mod render;
mod audio;
mod encode;
mod upload;
mod metadata;
mod text_util;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "urantia-render")]
#[command(about = "Rust video renderer for UrantiaHub YouTube channel")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download audio MP3s from CDN
    Download {
        #[arg(long, default_value = "0-196")]
        papers: String,
        #[arg(long, default_value = "./output")]
        output_dir: PathBuf,
    },
    /// Build timing manifests from audio durations
    Manifest {
        #[arg(long, default_value = "0-196")]
        papers: String,
        #[arg(long)]
        manifest_path: Option<PathBuf>,
        #[arg(long, default_value = "./output")]
        output_dir: PathBuf,
    },
    /// Render paper video(s) to MP4
    Render {
        #[arg(long, default_value = "0-196")]
        papers: String,
        #[arg(long, default_value = "./output")]
        output_dir: PathBuf,
        #[arg(long, default_value_t = num_cpus())]
        concurrency: usize,
        #[arg(long)]
        preview: bool,
        #[arg(long)]
        skip_existing: bool,
        /// Audio directory (supports nested {paperId}/ or flat tts-1-hd-nova-{id}.mp3 layout)
        #[arg(long)]
        audio_dir: Option<PathBuf>,
    },
    /// Generate YouTube metadata JSON
    Metadata {
        #[arg(long, default_value = "0-196")]
        papers: String,
        #[arg(long, default_value = "./output")]
        output_dir: PathBuf,
    },
    /// Upload MP4s to R2
    Upload {
        #[arg(long, default_value = "0-196")]
        papers: String,
        #[arg(long, default_value = "./output")]
        output_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        force: bool,
    },
    /// Trim outro branding from videos for CDN distribution
    TrimOutro {
        #[arg(long, default_value = "0-196")]
        papers: String,
        #[arg(long, default_value = "./output/videos")]
        input_dir: PathBuf,
        #[arg(long, default_value = "./output-cdn/videos")]
        output_dir: PathBuf,
        #[arg(long, default_value = "./output/manifests")]
        manifests_dir: PathBuf,
        #[arg(long)]
        skip_existing: bool,
    },
    /// Generate thumbnail PNGs with large text
    Thumbnail {
        #[arg(long, default_value = "0-196")]
        papers: String,
        #[arg(long, default_value = "./output/thumbnails")]
        output_dir: PathBuf,
    },
    /// Render the YouTube channel banner (2560x1440 PNG)
    Banner {
        #[arg(long, default_value = "./output/banner.png")]
        output: PathBuf,
    },
    /// Render the YouTube channel profile picture
    ChannelIcon {
        #[arg(long, default_value = "./output/channel-icon.png")]
        output: PathBuf,
        #[arg(long, default_value_t = 1024)]
        size: u32,
    },
    /// Render the 5 YouTube playlist thumbnails (master + Parts I-IV)
    PlaylistThumbnails {
        #[arg(long, default_value = "./output/thumbnails")]
        output_dir: PathBuf,
    },
    /// Render channel trailer (~60s)
    Trailer {
        #[arg(long, default_value = "./output/videos/trailer.mp4")]
        output: PathBuf,
        #[arg(long, default_value = "./output")]
        output_dir: PathBuf,
        #[arg(long)]
        manifest_path: Option<PathBuf>,
    },
    /// Run full pipeline
    All {
        #[arg(long, default_value = "0-196")]
        papers: String,
        #[arg(long, default_value = "./output")]
        output_dir: PathBuf,
        #[arg(long, default_value_t = num_cpus())]
        concurrency: usize,
    },
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get() / 2)
        .unwrap_or(2)
        .max(1)
}

fn parse_paper_range(range: &str) -> Vec<u32> {
    if range.contains(',') {
        // Comma-separated: "0,15,20,22"
        range.split(',').filter_map(|s| s.trim().parse().ok()).collect()
    } else if range.contains('-') {
        // Range: "0-196"
        let parts: Vec<&str> = range.split('-').collect();
        let start: u32 = parts[0].parse().unwrap_or(0);
        let end: u32 = parts[1].parse().unwrap_or(196);
        (start..=end).collect()
    } else {
        // Single: "1"
        vec![range.parse().unwrap_or(0)]
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
        Commands::Download { papers, output_dir } => {
            cmd_download(&papers, &output_dir).await?;
        }
        Commands::Manifest {
            papers,
            manifest_path,
            output_dir,
        } => {
            cmd_manifest(&papers, manifest_path.as_deref(), &output_dir).await?;
        }
        Commands::Render {
            papers,
            output_dir,
            skip_existing,
            preview,
            concurrency,
            audio_dir,
        } => {
            cmd_render(&papers, &output_dir, skip_existing, preview, concurrency, audio_dir.as_deref()).await?;
        }
        Commands::Metadata {
            papers,
            output_dir,
        } => {
            cmd_metadata(&papers, &output_dir).await?;
        }
        Commands::Upload {
            papers,
            output_dir,
            dry_run,
            force,
        } => {
            cmd_upload(&papers, &output_dir, dry_run, force).await?;
        }
        Commands::TrimOutro {
            papers,
            input_dir,
            output_dir,
            manifests_dir,
            skip_existing,
        } => {
            cmd_trim_outro(&papers, &input_dir, &output_dir, &manifests_dir, skip_existing).await?;
        }
        Commands::Thumbnail { papers, output_dir } => {
            cmd_thumbnails(&papers, &output_dir).await?;
        }
        Commands::Banner { output } => {
            cmd_banner(&output).await?;
        }
        Commands::ChannelIcon { output, size } => {
            cmd_channel_icon(&output, size).await?;
        }
        Commands::PlaylistThumbnails { output_dir } => cmd_playlist_thumbnails(&output_dir).await?,
        Commands::Trailer {
            output,
            output_dir,
            manifest_path,
        } => {
            cmd_trailer(&output, &output_dir, manifest_path.as_deref()).await?;
        }
        Commands::All { papers, .. } => {
            println!("Full pipeline not yet implemented");
            let _ = papers;
        }
    }

    Ok(())
}

async fn cmd_download(papers: &str, output_dir: &PathBuf) -> Result<()> {
    let paper_ids = parse_paper_range(papers);
    let audio_dir = output_dir.join("audio");

    println!("Downloading audio for {} papers...", paper_ids.len());

    for paper_id in &paper_ids {
        let url = config::paper_cdn_url(&paper_id.to_string());
        let resp = reqwest::get(&url).await?;
        let json = resp.text().await?;
        let paper = data::paper::Paper::from_json(&json)?;

        let (downloaded, skipped) =
            audio::download::download_paper_audio(&paper, &audio_dir).await?;
        println!(
            "  Paper {}: {} downloaded, {} skipped",
            paper_id, downloaded, skipped
        );
    }

    println!("Done!");
    Ok(())
}

async fn cmd_manifest(
    papers: &str,
    manifest_path: Option<&std::path::Path>,
    output_dir: &PathBuf,
) -> Result<()> {
    let paper_ids = parse_paper_range(papers);

    // Load audio manifest
    let audio_manifest = if let Some(path) = manifest_path {
        println!("Loading audio manifest from {:?}...", path);
        data::audio_manifest::AudioManifest::from_file(path)?
    } else {
        println!("Downloading audio manifest from CDN...");
        let resp = reqwest::get(config::MANIFEST_CDN_URL).await?;
        let json = resp.text().await?;
        data::audio_manifest::AudioManifest::from_json(&json)?
    };

    println!("Audio manifest: {} entries", audio_manifest.entry_count());

    let manifests_dir = output_dir.join("manifests");
    std::fs::create_dir_all(&manifests_dir)?;

    for paper_id in &paper_ids {
        let url = config::paper_cdn_url(&paper_id.to_string());
        let resp = reqwest::get(&url).await?;
        let json = resp.text().await?;
        let paper = data::paper::Paper::from_json(&json)?;

        let manifest = data::manifest::build_manifest(&paper, &audio_manifest);

        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        let manifest_file = manifests_dir.join(format!("{}.json", paper_id));
        std::fs::write(&manifest_file, &manifest_json)?;

        println!(
            "  Paper {}: {} segments, {}min",
            paper_id,
            manifest.segments.len(),
            manifest.total_duration_sec / 60
        );
    }

    println!("Done!");
    Ok(())
}

fn render_single_paper(
    paper_id: u32,
    manifests_dir: &std::path::Path,
    videos_dir: &std::path::Path,
    audio_dir: &std::path::Path,
    skip_existing: bool,
    preview: bool,
) -> Result<()> {
    let manifest_path = manifests_dir.join(format!("{}.json", paper_id));
    if !manifest_path.exists() {
        eprintln!("  Skipping Paper {}: no manifest. Run `manifest` first.", paper_id);
        return Ok(());
    }

    let manifest: data::manifest::PaperManifest =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)?;

    let video_name = config::video_filename(&paper_id.to_string());
    let output_path = videos_dir.join(&video_name);

    if skip_existing && output_path.exists() {
        let size = std::fs::metadata(&output_path)?.len();
        if size > 1000 {
            println!(
                "  Skipping Paper {}: already rendered ({:.1} MB)",
                paper_id,
                size as f64 / 1024.0 / 1024.0
            );
            return Ok(());
        }
    }

    let minutes = manifest.total_duration_sec / 60;
    println!(
        "  Paper {}: \"{}\" ({}min, {} segments)",
        paper_id, manifest.paper_title, minutes, manifest.segments.len()
    );

    let start = std::time::Instant::now();

    // Build audio PCM buffer
    let (pcm, sample_rate) = audio::concat::build_audio_buffer(&manifest, audio_dir)?;
    let wav_path = std::env::temp_dir().join(format!("urantia_paper_{}.wav", paper_id));
    audio::concat::write_wav(&pcm, sample_rate, &wav_path)?;

    // Render frames + encode
    let max_frames = if preview { Some(300) } else { None }; // ~10s preview
    render::pipeline::render_paper(&manifest, &output_path, &wav_path, max_frames)?;

    // Clean up temp WAV
    let _ = std::fs::remove_file(&wav_path);

    let elapsed = start.elapsed().as_secs();
    let size_mb = std::fs::metadata(&output_path)?.len() as f64 / 1024.0 / 1024.0;
    println!(
        "  Done: Paper {} — {} ({:.1} MB, {}s)",
        paper_id,
        output_path.display(),
        size_mb,
        elapsed
    );

    Ok(())
}

async fn cmd_render(
    papers: &str,
    output_dir: &PathBuf,
    skip_existing: bool,
    preview: bool,
    concurrency: usize,
    audio_dir_override: Option<&std::path::Path>,
) -> Result<()> {
    use rayon::prelude::*;

    let paper_ids = parse_paper_range(papers);
    let manifests_dir = output_dir.join("manifests");
    let videos_dir = output_dir.join("videos");
    let audio_dir = audio_dir_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| output_dir.join("audio"));

    std::fs::create_dir_all(&videos_dir)?;

    println!(
        "Rendering {} papers (concurrency: {})...",
        paper_ids.len(),
        concurrency
    );

    // Configure rayon thread pool
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(concurrency)
        .build()
        .unwrap();

    pool.install(|| {
        paper_ids.par_iter().for_each(|paper_id| {
            if let Err(e) = render_single_paper(
                *paper_id,
                &manifests_dir,
                &videos_dir,
                &audio_dir,
                skip_existing,
                preview,
            ) {
                eprintln!("  Error rendering Paper {}: {}", paper_id, e);
            }
        });
    });

    println!("All renders complete!");
    Ok(())
}

async fn cmd_metadata(papers: &str, output_dir: &PathBuf) -> Result<()> {
    let paper_ids = parse_paper_range(papers);
    let manifests_dir = output_dir.join("manifests");
    let metadata_dir = output_dir.join("metadata");
    std::fs::create_dir_all(&metadata_dir)?;

    println!("Generating metadata for {} papers...", paper_ids.len());

    let mut playlist = Vec::new();

    for paper_id in &paper_ids {
        let manifest_path = manifests_dir.join(format!("{}.json", paper_id));
        if !manifest_path.exists() {
            eprintln!("  Skipping Paper {}: no manifest.", paper_id);
            continue;
        }

        let manifest: data::manifest::PaperManifest =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)?;

        let meta = metadata::youtube::generate_and_write(&manifest, &metadata_dir)?;
        println!("  Paper {}: \"{}\"", paper_id, meta.title);
        playlist.push(meta);
    }

    // Write playlist manifest
    let playlist_json = serde_json::to_string_pretty(&playlist)?;
    std::fs::write(metadata_dir.join("playlist.json"), &playlist_json)?;
    println!("\nPlaylist manifest: {} videos", playlist.len());
    println!("Done!");
    Ok(())
}

async fn cmd_upload(
    papers: &str,
    output_dir: &PathBuf,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let paper_ids = parse_paper_range(papers);
    let videos_dir = output_dir.join("videos");
    let thumbnails_dir = output_dir.join("thumbnails");

    println!(
        "{}Uploading {} papers to R2 (videos + thumbnails)...",
        if dry_run { "[DRY RUN] " } else { "" },
        paper_ids.len()
    );

    let mut videos_uploaded = 0;
    let mut thumbs_uploaded = 0;
    let mut skipped = 0;

    for paper_id in &paper_ids {
        let pid = paper_id.to_string();

        match upload::r2::upload_video(&pid, &videos_dir, force, dry_run).await? {
            Some(_) => videos_uploaded += 1,
            None => {
                eprintln!("  Skipping Paper {}: video not found", paper_id);
                skipped += 1;
            }
        }

        if upload::r2::upload_thumbnail(&pid, &thumbnails_dir, force, dry_run).await?.is_some() {
            thumbs_uploaded += 1;
        }
    }

    println!(
        "\n{} videos, {} thumbnails uploaded. {} skipped.",
        videos_uploaded, thumbs_uploaded, skipped
    );
    Ok(())
}

async fn cmd_trim_outro(
    papers: &str,
    input_dir: &PathBuf,
    output_dir: &PathBuf,
    manifests_dir: &PathBuf,
    skip_existing: bool,
) -> Result<()> {
    let paper_ids = parse_paper_range(papers);
    std::fs::create_dir_all(output_dir)?;

    println!("Trimming outro from {} papers...", paper_ids.len());
    println!("  Input:  {}", input_dir.display());
    println!("  Output: {}", output_dir.display());

    let mut trimmed = 0;
    let mut skipped = 0;

    for paper_id in &paper_ids {
        let video_name = config::video_filename(&paper_id.to_string());
        let input_path = input_dir.join(&video_name);
        let output_path = output_dir.join(&video_name);

        if !input_path.exists() {
            eprintln!("  Skipping Paper {}: video not found", paper_id);
            skipped += 1;
            continue;
        }

        if skip_existing && output_path.exists() {
            let size = std::fs::metadata(&output_path)?.len();
            if size > 1000 {
                skipped += 1;
                continue;
            }
        }

        // Get actual video duration from ffprobe (more precise than manifest integer)
        let ffprobe_output = std::process::Command::new("ffprobe")
            .args([
                "-v", "quiet",
                "-show_entries", "format=duration",
                "-of", "default=noprint_wrappers=1:nokey=1",
                &input_path.to_string_lossy(),
            ])
            .output()
            .context("Failed to run ffprobe")?;

        let duration_sec: f64 = String::from_utf8_lossy(&ffprobe_output.stdout)
            .trim()
            .parse()
            .unwrap_or(0.0);

        if duration_sec <= 0.0 {
            eprintln!("  Skipping Paper {}: could not determine duration", paper_id);
            skipped += 1;
            continue;
        };

        let trim_to = duration_sec - 5.0; // remove 5s outro
        if trim_to <= 15.0 {
            eprintln!("  Skipping Paper {}: too short to trim", paper_id);
            skipped += 1;
            continue;
        }

        // Hybrid trim: stream copy the bulk, re-encode only the last ~10s
        let split_at = duration_sec - 15.0;
        let tail_duration = trim_to - split_at;
        let tmp_dir = std::env::temp_dir();
        let part1 = tmp_dir.join(format!("trim_part1_{}.mp4", paper_id));
        let part2 = tmp_dir.join(format!("trim_part2_{}.mp4", paper_id));
        let concat_list = tmp_dir.join(format!("trim_concat_{}.txt", paper_id));

        // Part 1: stream copy everything up to split point (instant)
        let s1 = std::process::Command::new("ffmpeg")
            .args([
                "-y", "-i", &input_path.to_string_lossy(),
                "-t", &format!("{:.3}", split_at),
                "-c", "copy",
                &part1.to_string_lossy(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .context("ffmpeg part1 failed")?;

        // Part 2: re-encode only the last ~10s with precise end point
        let s2 = std::process::Command::new("ffmpeg")
            .args([
                "-y", "-i", &input_path.to_string_lossy(),
                "-ss", &format!("{:.3}", split_at),
                "-t", &format!("{:.3}", tail_duration),
                "-c:v", "libx264", "-preset", "medium", "-crf", "20", "-pix_fmt", "yuv420p",
                "-c:a", "aac", "-b:a", "128k",
                &part2.to_string_lossy(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .context("ffmpeg part2 failed")?;

        // Concatenate
        std::fs::write(&concat_list, format!(
            "file '{}'\nfile '{}'",
            part1.to_string_lossy(),
            part2.to_string_lossy(),
        ))?;

        let s3 = std::process::Command::new("ffmpeg")
            .args([
                "-y", "-f", "concat", "-safe", "0",
                "-i", &concat_list.to_string_lossy(),
                "-c", "copy", "-movflags", "+faststart",
                &output_path.to_string_lossy(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .context("ffmpeg concat failed")?;

        // Clean up temp files
        let _ = std::fs::remove_file(&part1);
        let _ = std::fs::remove_file(&part2);
        let _ = std::fs::remove_file(&concat_list);

        if s1.success() && s2.success() && s3.success() {
            trimmed += 1;
            if trimmed % 20 == 0 || trimmed == 1 {
                println!("  Trimmed {}/{}", trimmed, paper_ids.len());
            }
        } else {
            eprintln!("  Error trimming Paper {}", paper_id);
            skipped += 1;
        }
    }

    println!("\n{} trimmed, {} skipped.", trimmed, skipped);
    Ok(())
}

async fn cmd_banner(output: &PathBuf) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    println!("Rendering YouTube channel banner (2560x1440)...");

    let mut renderer = render::text::TextRenderer::new();

    const BW: u32 = 2560;
    const BH: u32 = 1440;
    let mut pixmap = tiny_skia::Pixmap::new(BW, BH).unwrap();
    {
        let data = pixmap.data_mut();
        for i in (0..data.len()).step_by(4) {
            data[i]     = config::BG_COLOR[0];
            data[i + 1] = config::BG_COLOR[1];
            data[i + 2] = config::BG_COLOR[2];
            data[i + 3] = config::BG_COLOR[3];
        }
    }

    render::cards::render_banner(&mut renderer, &mut pixmap);
    pixmap.save_png(output)?;

    println!("  → {}", output.display());
    Ok(())
}

async fn cmd_channel_icon(output: &PathBuf, size: u32) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    println!("Rendering channel icon ({size}x{size})...");

    let mut pixmap = tiny_skia::Pixmap::new(size, size).unwrap();
    {
        let data = pixmap.data_mut();
        for i in (0..data.len()).step_by(4) {
            data[i]     = config::BG_COLOR[0];
            data[i + 1] = config::BG_COLOR[1];
            data[i + 2] = config::BG_COLOR[2];
            data[i + 3] = config::BG_COLOR[3];
        }
    }

    render::cards::render_channel_icon(&mut pixmap);
    pixmap.save_png(output)?;

    println!("  → {}", output.display());
    Ok(())
}

async fn cmd_playlist_thumbnails(output_dir: &PathBuf) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;

    println!("Rendering 5 playlist thumbnails (1920x1080)...");

    let mut renderer = render::text::TextRenderer::new();

    let build_canvas = || -> tiny_skia::Pixmap {
        let mut pixmap = tiny_skia::Pixmap::new(1920, 1080).unwrap();
        {
            let data = pixmap.data_mut();
            for i in (0..data.len()).step_by(4) {
                data[i]     = config::BG_COLOR[0];
                data[i + 1] = config::BG_COLOR[1];
                data[i + 2] = config::BG_COLOR[2];
                data[i + 3] = config::BG_COLOR[3];
            }
        }
        pixmap
    };

    // Master playlist (all 197)
    {
        let mut pixmap = build_canvas();
        render::cards::render_playlist_thumbnail_with_subtitle(
            &mut renderer,
            &mut pixmap,
            "",
            "All 197 Papers",
            Some("Audio and text, read along"),
        );
        let out = output_dir.join("playlist-all.png");
        pixmap.save_png(&out)?;
        println!("  → {}", out.display());
    }

    // Parts I–IV
    let parts = [
        ("Part I",   "The Central and\nSuperuniverses",      "playlist-part-1"),
        ("Part II",  "The Local Universe",                    "playlist-part-2"),
        ("Part III", "The History\nof Urantia",               "playlist-part-3"),
        ("Part IV",  "The Life and Teachings\nof Jesus",      "playlist-part-4"),
    ];
    for (label, title, file_stem) in parts.iter() {
        let mut pixmap = build_canvas();
        render::cards::render_playlist_thumbnail(&mut renderer, &mut pixmap, label, title);
        let out = output_dir.join(format!("{}.png", file_stem));
        pixmap.save_png(&out)?;
        println!("  → {}", out.display());
    }

    println!("Done!");
    Ok(())
}

async fn cmd_thumbnails(papers: &str, output_dir: &PathBuf) -> Result<()> {
    let paper_ids = parse_paper_range(papers);
    std::fs::create_dir_all(output_dir)?;

    println!("Generating {} thumbnails...", paper_ids.len());

    let mut renderer = render::text::TextRenderer::new();

    for paper_id in &paper_ids {
        let url = config::paper_cdn_url(&paper_id.to_string());
        let resp = reqwest::get(&url).await?;
        let json = resp.text().await?;
        let paper = data::paper::Paper::from_json(&json)?;

        // Thumbnails are YouTube browse-view assets — keep at 1920×1080 regardless
        // of the video canvas resolution. Build a dark fill directly rather than
        // calling render_background (which uses config::WIDTH/HEIGHT and now
        // produces 4K output).
        let mut pixmap = tiny_skia::Pixmap::new(1920, 1080).unwrap();
        {
            let data = pixmap.data_mut();
            for i in (0..data.len()).step_by(4) {
                data[i]     = config::BG_COLOR[0];
                data[i + 1] = config::BG_COLOR[1];
                data[i + 2] = config::BG_COLOR[2];
                data[i + 3] = config::BG_COLOR[3];
            }
        }
        let mut content = tiny_skia::Pixmap::new(1920, 1080).unwrap();
        render::cards::render_thumbnail(&mut renderer, &mut content, &paper.paper_id, &paper.paper_title);
        render::compositor::composite(&mut pixmap, &content, 1.0);

        let output_path = output_dir.join(format!("thumbnail-{}.png", paper_id));
        pixmap.save_png(&output_path)?;
        println!("  Paper {}: {}", paper_id, output_path.display());
    }

    println!("Done!");
    Ok(())
}

async fn cmd_trailer(
    output: &PathBuf,
    output_dir: &PathBuf,
    manifest_path: Option<&std::path::Path>,
) -> Result<()> {
    use crate::data::manifest::{PaperManifest, Segment};
    use crate::data::text_chunker::TextChunk;

    println!("Building channel trailer...");

    // Load audio manifest for durations
    let audio_manifest = if let Some(path) = manifest_path {
        data::audio_manifest::AudioManifest::from_file(path)?
    } else {
        println!("  Downloading audio manifest from CDN...");
        let resp = reqwest::get(config::MANIFEST_CDN_URL).await?;
        let json = resp.text().await?;
        data::audio_manifest::AudioManifest::from_json(&json)?
    };

    // Load Paper 1 JSON for paragraph text
    let url = config::paper_cdn_url("1");
    let resp = reqwest::get(&url).await?;
    let json = resp.text().await?;
    let paper = data::paper::Paper::from_json(&json)?;

    // Find paragraphs 1:0.1 and 1:0.3
    let para_0_1 = paper.sections.iter()
        .flat_map(|s| &s.paragraphs)
        .find(|p| p.global_id == "1:1.0.1")
        .ok_or_else(|| anyhow::anyhow!("Paragraph 1:1.0.1 not found"))?;

    let para_0_3 = paper.sections.iter()
        .flat_map(|s| &s.paragraphs)
        .find(|p| p.global_id == "1:1.0.3")
        .ok_or_else(|| anyhow::anyhow!("Paragraph 1:1.0.3 not found"))?;

    // Get audio durations
    let intro_gid = "1:1.-.-";
    let intro_dur = audio_manifest.get_duration(intro_gid).unwrap_or(2.0);
    let dur_0_1 = audio_manifest.get_duration("1:1.0.1").unwrap_or(45.0);
    let dur_0_3 = audio_manifest.get_duration("1:1.0.3").unwrap_or(60.0);

    let fps = config::FPS;
    let mut current_frame = 0u32;

    // Build custom manifest
    let mut segments = Vec::new();

    // 1. Intro card (Paper 1 title) — audio duration + 1s padding
    let intro_frames = (intro_dur * fps as f64).ceil() as u32 + fps;
    segments.push(Segment::Intro {
        paper_title: "The Universal Father".to_string(),
        paper_id: "1".to_string(),
        start_frame: current_frame,
        duration_frames: intro_frames,
    });
    current_frame += intro_frames;

    // 2. Paragraph 1:0.1
    let frames_0_1 = (dur_0_1 * fps as f64).ceil() as u32;
    segments.push(Segment::Paragraph {
        global_id: "1:1.0.1".to_string(),
        standard_reference_id: para_0_1.standard_reference_id.clone(),
        text: para_0_1.text.clone(),
        section_title: None,
        audio_duration_sec: dur_0_1,
        start_frame: current_frame,
        duration_frames: frames_0_1,
        text_chunks: vec![TextChunk {
            text: para_0_1.text.clone(),
            start_frame: 0,
            duration_frames: frames_0_1,
        }],
    });
    current_frame += frames_0_1;

    // 3. Paragraph 1:0.3
    let frames_0_3 = (dur_0_3 * fps as f64).ceil() as u32;
    let chunks_0_3 = data::text_chunker::chunk_text(&para_0_3.text, dur_0_3, frames_0_3);
    segments.push(Segment::Paragraph {
        global_id: "1:1.0.3".to_string(),
        standard_reference_id: para_0_3.standard_reference_id.clone(),
        text: para_0_3.text.clone(),
        section_title: None,
        audio_duration_sec: dur_0_3,
        start_frame: current_frame,
        duration_frames: frames_0_3,
        text_chunks: chunks_0_3,
    });
    current_frame += frames_0_3;

    // 4. Outro with custom tagline
    let outro_frames = config::OUTRO_FRAMES;
    segments.push(Segment::Outro {
        start_frame: current_frame,
        duration_frames: outro_frames,
        tagline: Some("197 Papers. Every paragraph. Listen and read along.".to_string()),
    });
    current_frame += outro_frames;

    let manifest = PaperManifest {
        paper_id: "1".to_string(), // use "1" so audio lookup finds files in output/audio/1/
        paper_title: "Channel Trailer".to_string(),
        part_id: "1".to_string(),
        fps,
        segments,
        total_duration_frames: current_frame,
        total_duration_sec: current_frame / fps,
    };

    println!(
        "  Trailer: {} segments, {}s total",
        manifest.segments.len(),
        manifest.total_duration_sec
    );

    // Download audio for trailer paragraphs
    let audio_dir = output_dir.join("audio").join("1");
    tokio::fs::create_dir_all(&audio_dir).await?;

    let client = reqwest::Client::new();
    for gid in &[intro_gid, "1:1.0.1", "1:1.0.3"] {
        let dest = audio_dir.join(format!("{}.mp3", gid));
        if !dest.exists() {
            let url = config::audio_url(gid);
            let bytes = client.get(&url).send().await?.bytes().await?;
            tokio::fs::write(&dest, &bytes).await?;
            println!("  Downloaded {}", gid);
        }
    }

    // Build audio buffer
    eprint!("  Building audio buffer...");
    let (pcm, sample_rate) = audio::concat::build_audio_buffer(&manifest, &output_dir.join("audio"))?;
    let wav_path = std::env::temp_dir().join("urantia_trailer.wav");
    audio::concat::write_wav(&pcm, sample_rate, &wav_path)?;
    eprintln!(" done ({:.1}s)", pcm.len() as f64 / sample_rate as f64);

    // Render
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let start = std::time::Instant::now();
    render::pipeline::render_paper(&manifest, output, &wav_path, None)?;
    let _ = std::fs::remove_file(&wav_path);

    let elapsed = start.elapsed().as_secs();
    let size_mb = std::fs::metadata(output)?.len() as f64 / 1024.0 / 1024.0;
    println!(
        "  Done: {} ({:.1} MB, {}s)",
        output.display(),
        size_mb,
        elapsed
    );

    Ok(())
}
