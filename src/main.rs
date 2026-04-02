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
        Commands::Render { papers, .. } => {
            println!("Render not yet implemented (Phase 2-3)");
            let _ = papers;
        }
        Commands::Metadata { papers, .. } => {
            println!("Metadata not yet implemented (Phase 4)");
            let _ = papers;
        }
        Commands::Upload { papers, .. } => {
            println!("Upload not yet implemented (Phase 4)");
            let _ = papers;
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
