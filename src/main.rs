mod config;
mod data;
mod render;
mod audio;
mod encode;
mod upload;
mod metadata;

use anyhow::Result;
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
    if range.contains('-') {
        let parts: Vec<&str> = range.split('-').collect();
        let start: u32 = parts[0].parse().unwrap_or(0);
        let end: u32 = parts[1].parse().unwrap_or(196);
        (start..=end).collect()
    } else {
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
            ..
        } => {
            cmd_render(&papers, &output_dir, skip_existing, preview).await?;
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

async fn cmd_render(
    papers: &str,
    output_dir: &PathBuf,
    skip_existing: bool,
    _preview: bool,
) -> Result<()> {
    let paper_ids = parse_paper_range(papers);
    let manifests_dir = output_dir.join("manifests");
    let videos_dir = output_dir.join("videos");
    let audio_dir = output_dir.join("audio");

    std::fs::create_dir_all(&videos_dir)?;

    println!("Rendering {} papers...", paper_ids.len());

    for paper_id in &paper_ids {
        let manifest_path = manifests_dir.join(format!("{}.json", paper_id));
        if !manifest_path.exists() {
            eprintln!("  Skipping Paper {}: no manifest. Run `manifest` first.", paper_id);
            continue;
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
                continue;
            }
        }

        let minutes = manifest.total_duration_sec / 60;
        println!(
            "  Paper {}: \"{}\" ({}min, {} segments)",
            paper_id, manifest.paper_title, minutes, manifest.segments.len()
        );

        let start = std::time::Instant::now();

        // Build audio PCM buffer
        eprint!("  Building audio buffer...");
        let (pcm, sample_rate) = audio::concat::build_audio_buffer(&manifest, &audio_dir)?;
        let wav_path = std::env::temp_dir().join(format!("urantia_paper_{}.wav", paper_id));
        audio::concat::write_wav(&pcm, sample_rate, &wav_path)?;
        eprintln!(" done ({:.1}s audio)", pcm.len() as f64 / sample_rate as f64);

        // Render frames + encode
        let max_frames = if _preview { Some(300) } else { None }; // 10s preview
        render::pipeline::render_paper(&manifest, &output_path, &wav_path, max_frames)?;

        // Clean up temp WAV
        let _ = std::fs::remove_file(&wav_path);

        let elapsed = start.elapsed().as_secs();
        let size_mb = std::fs::metadata(&output_path)?.len() as f64 / 1024.0 / 1024.0;
        println!(
            "  Done: {} ({:.1} MB, {}s)",
            output_path.display(),
            size_mb,
            elapsed
        );
    }

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

    println!(
        "{}Uploading {} papers to R2...",
        if dry_run { "[DRY RUN] " } else { "" },
        paper_ids.len()
    );

    let mut uploaded = 0;
    let mut skipped = 0;

    for paper_id in &paper_ids {
        match upload::r2::upload_video(&paper_id.to_string(), &videos_dir, force, dry_run).await? {
            Some(_) => uploaded += 1,
            None => {
                eprintln!("  Skipping Paper {}: video not found", paper_id);
                skipped += 1;
            }
        }
    }

    println!("\n{} uploaded, {} skipped.", uploaded, skipped);
    Ok(())
}
