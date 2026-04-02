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
    /// Generate thumbnail PNGs with large text
    Thumbnail {
        #[arg(long, default_value = "0-196")]
        papers: String,
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
        Commands::Thumbnail { papers, output_dir } => {
            cmd_thumbnails(&papers, &output_dir).await?;
        }
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

        let mut pixmap = render::background::render_background(2.5);
        let mut content = tiny_skia::Pixmap::new(config::WIDTH, config::HEIGHT).unwrap();
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
