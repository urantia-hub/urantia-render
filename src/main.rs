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
        /// Stop rendering after N seconds. Useful for dev iteration.
        #[arg(long)]
        max_seconds: Option<u32>,
        #[arg(long)]
        skip_existing: bool,
        /// Audio directory (supports nested {paperId}/ or flat tts-1-hd-nova-{id}.mp3 layout)
        #[arg(long)]
        audio_dir: Option<PathBuf>,
    },
    /// Build the WAV audio track for paper(s), skipping the video pipeline.
    /// Used for remediating previously-rendered silent MP4s: produce this
    /// WAV, then use ffmpeg to swap it into the existing MP4 without
    /// re-encoding video.
    AudioOnly {
        #[arg(long, default_value = "0-196")]
        papers: String,
        #[arg(long, default_value = "./output")]
        output_dir: PathBuf,
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
    /// Render just the 5-second outro card as a standalone MP4 (dev preview).
    OutroPreview {
        #[arg(long, default_value = "./output/videos/outro-preview.mp4")]
        output: PathBuf,
    },
    /// Render the 30s channel trailer (cold open + 4 part headers + CTA + outro)
    Trailer {
        #[arg(long, default_value = "./output/videos/channel-trailer.mp4")]
        output: PathBuf,
        /// Directory containing trailer/audio/{cold-open,part-1..4,cta}.mp3 + trailer/music/bed.mp3
        #[arg(long, default_value = "./output")]
        output_dir: PathBuf,
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
            max_seconds,
            concurrency,
            audio_dir,
        } => {
            cmd_render(&papers, &output_dir, skip_existing, preview, max_seconds, concurrency, audio_dir.as_deref()).await?;
        }
        Commands::AudioOnly {
            papers,
            output_dir,
            audio_dir,
        } => {
            cmd_audio_only(&papers, &output_dir, audio_dir.as_deref())?;
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
        Commands::OutroPreview { output } => cmd_outro_preview(&output).await?,
        Commands::Trailer { output, output_dir } => {
            cmd_trailer(&output, &output_dir).await?;
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
    max_seconds: Option<u32>,
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
    let max_frames = match (preview, max_seconds) {
        (true, _) => Some(300),             // ~10s preview
        (_, Some(s)) => Some(s * config::FPS), // explicit duration cap
        _ => None,
    };
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
    max_seconds: Option<u32>,
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
                max_seconds,
            ) {
                eprintln!("  Error rendering Paper {}: {}", paper_id, e);
            }
        });
    });

    println!("All renders complete!");
    Ok(())
}

fn audio_only_single_paper(
    paper_id: u32,
    manifests_dir: &std::path::Path,
    wav_dir: &std::path::Path,
    audio_dir: &std::path::Path,
) -> Result<()> {
    let manifest_path = manifests_dir.join(format!("{}.json", paper_id));
    if !manifest_path.exists() {
        eprintln!("  Skipping Paper {}: no manifest. Run `manifest` first.", paper_id);
        return Ok(());
    }

    let manifest: data::manifest::PaperManifest =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)?;

    let wav_path = wav_dir.join(format!("{}.wav", paper_id));
    println!(
        "  Paper {}: \"{}\" -> {}",
        paper_id, manifest.paper_title, wav_path.display()
    );

    let start = std::time::Instant::now();
    let (pcm, sample_rate) = audio::concat::build_audio_buffer(&manifest, audio_dir)?;
    audio::concat::write_wav(&pcm, sample_rate, &wav_path)?;

    let elapsed = start.elapsed().as_secs();
    let size_mb = std::fs::metadata(&wav_path)?.len() as f64 / 1024.0 / 1024.0;
    println!(
        "  Done: Paper {} WAV ({:.1} MB, {}s, {} Hz)",
        paper_id, size_mb, elapsed, sample_rate
    );
    Ok(())
}

fn cmd_audio_only(
    papers: &str,
    output_dir: &PathBuf,
    audio_dir_override: Option<&std::path::Path>,
) -> Result<()> {
    use rayon::prelude::*;

    let paper_ids = parse_paper_range(papers);
    let manifests_dir = output_dir.join("manifests");
    let wav_dir = output_dir.join("wav");
    let audio_dir = audio_dir_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| output_dir.join("audio"));

    std::fs::create_dir_all(&wav_dir)?;

    println!("Building audio WAVs for {} papers...", paper_ids.len());

    paper_ids.par_iter().for_each(|paper_id| {
        if let Err(e) = audio_only_single_paper(*paper_id, &manifests_dir, &wav_dir, &audio_dir) {
            eprintln!("  Error assembling audio for Paper {}: {}", paper_id, e);
        }
    });

    println!("All audio-only builds complete!");
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

        let meta = metadata::youtube::generate_and_write(&manifest, &metadata_dir).await?;
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

    // 2560×1440 banner with cosmic orbs. Scale 1.333 matches the 1920→2560
    // ratio so the orbs fill the frame similarly to the 1080p thumbnails.
    let mut pixmap = render::background::render_background_at(2560, 1440, 1.333, 2.5);

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

    // YouTube crops the profile icon to a circle, so orbs that drift beyond
    // the inscribed circle are clipped anyway. Render with a subtle orb glow
    // behind the logo for visual consistency with the banner/thumbnails.
    let scale = size as f32 / 1920.0;
    let mut pixmap = render::background::render_background_at(size, size, scale, 2.5);

    render::cards::render_channel_icon(&mut pixmap);
    pixmap.save_png(output)?;

    println!("  → {}", output.display());
    Ok(())
}

async fn cmd_outro_preview(output: &PathBuf) -> Result<()> {
    use crate::data::manifest::{PaperManifest, Segment};

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    println!("Rendering outro preview (5s standalone MP4)...");

    // Single-segment manifest: just the outro at frame 0.
    let manifest = PaperManifest {
        paper_id: "preview".to_string(),
        paper_title: "Outro Preview".to_string(),
        part_id: "0".to_string(),
        fps: config::FPS,
        segments: vec![Segment::Outro {
            start_frame: 0,
            duration_frames: config::OUTRO_FRAMES,
            tagline: None,
        }],
        total_duration_frames: config::OUTRO_FRAMES,
        total_duration_sec: (config::OUTRO_FRAMES / config::FPS) as u32,
    };

    // Silent WAV matching outro duration (the outro has no audio).
    let rate = audio::concat::SAMPLE_RATE;
    let total_samples = (config::OUTRO_SEC * rate as f64) as usize;
    let pcm = vec![0i16; total_samples];
    let wav_path = std::env::temp_dir().join("urantia_outro_preview.wav");
    audio::concat::write_wav(&pcm, rate, &wav_path)?;

    render::pipeline::render_paper(&manifest, output, &wav_path, None)?;
    let _ = std::fs::remove_file(&wav_path);

    println!("  → {}", output.display());
    Ok(())
}

async fn cmd_playlist_thumbnails(output_dir: &PathBuf) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;

    println!("Rendering 5 playlist thumbnails (3840x2160)...");

    let mut renderer = render::text::TextRenderer::new();

    // Master playlist (all 197)
    {
        let mut pixmap = render::background::render_background_at(3840, 2160, 2.0, 2.5);
        let mut content = tiny_skia::Pixmap::new(3840, 2160).unwrap();
        render::cards::render_playlist_thumbnail_with_subtitle(
            &mut renderer,
            &mut content,
            "",
            "All 197 Papers",
            Some("Audio and text,\nread along"),
        );
        render::compositor::composite(&mut pixmap, &content, 1.0);
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
    // Use a staggered time_sec per part so the orbs are in a different
    // position on each thumbnail — avoids all 4 parts looking identical.
    for (i, (label, title, file_stem)) in parts.iter().enumerate() {
        let time_sec = 2.5 + (i as f64) * 7.0;
        let mut pixmap = render::background::render_background_at(3840, 2160, 2.0, time_sec);
        let mut content = tiny_skia::Pixmap::new(3840, 2160).unwrap();
        render::cards::render_playlist_thumbnail(&mut renderer, &mut content, label, title);
        render::compositor::composite(&mut pixmap, &content, 1.0);
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

        // YouTube recommends 3840×2160 for custom thumbnails (with 16:9 aspect,
        // under 2MB on mobile / 50MB on desktop). Render at 4K with orb background.
        let mut pixmap = render::background::render_background_at(3840, 2160, 2.0, 2.5);
        let mut content = tiny_skia::Pixmap::new(3840, 2160).unwrap();
        render::cards::render_thumbnail(&mut renderer, &mut content, &paper.paper_id, &paper.paper_title);
        render::compositor::composite(&mut pixmap, &content, 1.0);

        let output_path = output_dir.join(format!("thumbnail-{}.png", paper_id));
        pixmap.save_png(&output_path)?;
        println!("  Paper {}: {}", paper_id, output_path.display());
    }

    println!("Done!");
    Ok(())
}

async fn cmd_trailer(output: &PathBuf, output_dir: &PathBuf) -> Result<()> {
    use crate::data::manifest::{PaperManifest, Segment};
    use std::process::Command;

    println!("Building channel trailer...");

    let fps = config::FPS;
    let mut current_frame = 0u32;
    let mut segments = Vec::new();

    let card = |title: &str, frames: u32, start: u32| Segment::SectionCard {
        section_title: title.to_string(),
        start_frame: start,
        duration_frames: frames,
    };

    // 1. Cold open (4.5s — narration is "The Urantia Papers." (~1.3s); the
    // remaining ~3.2s is a deliberate musical pause before the Part headers)
    let cold_frames = 9 * fps / 2;
    segments.push(card(
        "The Urantia Papers",
        cold_frames,
        current_frame,
    ));
    current_frame += cold_frames;

    // 2-5. Four part headers (4s each) — Roman numeral label + title on its own line
    let part_frames = 4 * fps;
    for (label, title) in [
        ("Part I", "The Central and Superuniverses"),
        ("Part II", "The Local Universe"),
        ("Part III", "The History of Urantia"),
        ("Part IV", "The Life and Teachings of Jesus"),
    ] {
        segments.push(card(
            &format!("{}\n{}", label, title),
            part_frames,
            current_frame,
        ));
        current_frame += part_frames;
    }

    // 6. CTA card (5s)
    let cta_frames = 5 * fps;
    segments.push(card(
        "Read along to every paper\nwhile you listen.",
        cta_frames,
        current_frame,
    ));
    current_frame += cta_frames;

    // 7. UrantiaHub outro (5s)
    let outro_frames = 5 * fps;
    segments.push(Segment::Outro {
        start_frame: current_frame,
        duration_frames: outro_frames,
        tagline: None,
    });
    current_frame += outro_frames;

    let manifest = PaperManifest {
        paper_id: "trailer".to_string(),
        paper_title: "Channel Trailer".to_string(),
        part_id: "trailer".to_string(),
        fps,
        segments,
        total_duration_frames: current_frame,
        total_duration_sec: current_frame / fps,
    };

    let total_sec = current_frame as f64 / fps as f64;
    println!(
        "  {} segments, {:.1}s total",
        manifest.segments.len(),
        total_sec
    );

    // Generate a silent WAV the renderer can mux as the placeholder track.
    // Real audio gets ffmpeg-overlaid in the post-processing step below.
    let silent_wav = std::env::temp_dir().join("urantia_trailer_silent.wav");
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=stereo",
            "-t",
            &total_sec.to_string(),
            silent_wav.to_str().unwrap(),
        ])
        .stderr(std::process::Stdio::null())
        .status()
        .context("ffmpeg silent-wav generation failed")?;
    if !status.success() {
        anyhow::bail!("ffmpeg silent-wav exited non-zero");
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let silent_mp4 = output.with_file_name("channel-trailer-silent.mp4");
    eprintln!("  Rendering visuals → {}", silent_mp4.display());
    let render_start = std::time::Instant::now();
    // hold_step=1 → full 30fps gradient (no orb stutter on long card holds).
    // Override the encoder to libx264 + CRF 16 for the trailer — Apple's
    // videotoolbox at q:v 65 produces macroblock artifacts on smooth
    // gradients when every frame is unique (no duplicate-frame compression
    // wins). Software libx264 at CRF 16 is slower but pristine. Restored
    // afterward so it doesn't leak into other renders in the same run.
    let prev_encoder = std::env::var("URANTIA_RENDER_ENCODER").ok();
    std::env::set_var("URANTIA_RENDER_ENCODER", "libx264-trailer");
    let result = render::pipeline::render_paper_with_options(
        &manifest,
        &silent_mp4,
        &silent_wav,
        None,
        1,
    );
    match prev_encoder {
        Some(v) => std::env::set_var("URANTIA_RENDER_ENCODER", v),
        None => std::env::remove_var("URANTIA_RENDER_ENCODER"),
    }
    result?;
    let _ = std::fs::remove_file(&silent_wav);
    eprintln!(
        "    rendered in {}s",
        render_start.elapsed().as_secs()
    );

    // Mix narration + music bed under the rendered visuals.
    // Narration offsets line up with each card: cold-open enters ~0.5s in (after
    // fade-in), each Part 0.5s after its card starts, CTA 0.5s after its card.
    let audio_dir = output_dir.join("trailer/audio");
    let bed = output_dir.join("trailer/music/bed.mp3");
    let need = [
        "cold-open.mp3",
        "part-1.mp3",
        "part-2.mp3",
        "part-3.mp3",
        "part-4.mp3",
        "cta.mp3",
        "outro.mp3",
    ];
    for f in &need {
        if !audio_dir.join(f).exists() {
            anyhow::bail!("missing narration clip: {}", audio_dir.join(f).display());
        }
    }
    if !bed.exists() {
        anyhow::bail!("missing music bed: {}", bed.display());
    }

    // Card start times (sec): 0, 4.5, 8.5, 12.5, 16.5, 20.5, 25.5
    // Narration enters 0.5s into each card so the fade-in lands first.
    let offsets_ms = [500u32, 5000, 9000, 13000, 17000, 21000, 26000];

    let mut filter = String::new();
    // Music bed at -15dB (~0.18 linear) so narration sits clearly on top.
    filter.push_str("[1:a]volume=0.18[bed];");
    for (i, ms) in offsets_ms.iter().enumerate() {
        // Inputs 2..=8 are the narration clips.
        filter.push_str(&format!("[{}:a]adelay={}|{}[v{}];", i + 2, ms, ms, i));
    }
    filter.push_str(
        "[bed][v0][v1][v2][v3][v4][v5][v6]amix=inputs=8:dropout_transition=0:normalize=0[a]",
    );

    eprintln!("  Mixing audio (narration + music bed) → {}", output.display());
    let mut args: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        silent_mp4.to_string_lossy().into_owned(),
        "-i".into(),
        bed.to_string_lossy().into_owned(),
    ];
    for f in &need {
        args.push("-i".into());
        args.push(audio_dir.join(f).to_string_lossy().into_owned());
    }
    args.extend([
        "-filter_complex".into(),
        filter,
        "-map".into(),
        "0:v".into(),
        "-map".into(),
        "[a]".into(),
        "-c:v".into(),
        "copy".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-shortest".into(),
        output.to_string_lossy().into_owned(),
    ]);

    let mix_status = Command::new("ffmpeg")
        .args(&args)
        .stderr(std::process::Stdio::null())
        .status()
        .context("ffmpeg audio-mix failed")?;
    if !mix_status.success() {
        anyhow::bail!("ffmpeg audio-mix exited non-zero");
    }
    let _ = std::fs::remove_file(&silent_mp4);

    let size_mb = std::fs::metadata(output)?.len() as f64 / 1024.0 / 1024.0;
    println!("  Done: {} ({:.1} MB)", output.display(), size_mb);

    // Generate the matching channel-trailer thumbnail (3840x2160) so it can
    // be uploaded alongside the video. Reuses the playlist-thumbnail layout
    // for visual consistency with the channel's existing thumbnails.
    let thumb_dir = output_dir.join("thumbnails");
    std::fs::create_dir_all(&thumb_dir)?;
    let thumb_path = thumb_dir.join("channel-trailer.png");
    let mut renderer = render::text::TextRenderer::new();
    let mut pixmap = render::background::render_background_at(3840, 2160, 2.0, 1.5);
    let mut content = tiny_skia::Pixmap::new(3840, 2160).unwrap();
    // Gold label "The Urantia Papers" (thumbnail_paper_number style: #D4A84A
    // Lato Bold) + white title "Listen & Read" (thumbnail_paper_title_right
    // style: Lora SemiBold).
    render::cards::render_playlist_thumbnail(
        &mut renderer,
        &mut content,
        "The Urantia Papers",
        "Listen & Read",
    );
    render::compositor::composite(&mut pixmap, &content, 1.0);
    pixmap.save_png(&thumb_path)?;
    println!("  Thumbnail: {}", thumb_path.display());

    Ok(())
}
