#!/bin/bash
# cloud-init user-data for a c7a.16xlarge (or similar) Ubuntu 24.04 spot instance.
# Bootstraps a full urantia-render batch of all 197 papers at 4K, writes MP4s
# to an S3 bucket in the same region, then shuts the instance down.
#
# Configure via instance tags or by editing the env block below before boot.
#
# Prereqs:
#   - Launched from an Ubuntu 24.04 LTS AMI (HVM, SSD) in us-east-2 ideally.
#   - IAM instance profile with s3:PutObject on the target bucket.
#   - Instance security group allows outbound 443 (GitHub, crates.io, cdn.urantia.dev).

set -euxo pipefail

# ─── Config (edit or inject via Terraform/cloud-init template) ───
REPO_URL="${REPO_URL:-https://github.com/urantia-hub/urantia-render.git}"
REPO_BRANCH="${REPO_BRANCH:-main}"
S3_BUCKET="${S3_BUCKET:?set S3_BUCKET env var}"
S3_PREFIX="${S3_PREFIX:-urantia-render/$(date -u +%Y-%m-%d)}"
CONCURRENCY="${CONCURRENCY:-12}"
THREADS_PER_FFMPEG="${THREADS_PER_FFMPEG:-4}"
AUDIO_MANIFEST_URL="${AUDIO_MANIFEST_URL:-https://cdn.urantia.dev/manifests/audio-manifest.json}"
# Use the full-detail manifest (has durations) — override if pulling from urantia-dev-api instead.
PAPER_RANGE="${PAPER_RANGE:-0-196}"

export DEBIAN_FRONTEND=noninteractive
export LOG=/var/log/urantia-render-batch.log
exec > >(tee -a "$LOG") 2>&1

echo "[$(date -u)] boot: begin"

# ─── 1. OS packages ───
apt-get update -qq
apt-get install -yqq \
    build-essential curl git pkg-config libssl-dev ca-certificates \
    ffmpeg awscli jq unzip numactl

# ─── 2. Rust toolchain (stable) ───
if ! command -v cargo >/dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
    . "$HOME/.cargo/env"
fi
. "$HOME/.cargo/env"

# ─── 3. Clone + build ───
cd /opt
if [ ! -d urantia-render ]; then
    git clone --depth 1 --branch "$REPO_BRANCH" "$REPO_URL"
fi
cd urantia-render
git pull || true

# Build release binary (uses libx264 on Linux via platform detection in ffmpeg.rs).
cargo build --release --bin urantia-render

# ─── 4. Pull the audio manifest + prime the output layout ───
mkdir -p output/manifests output/audio output/videos output/metadata

# Local audio manifest with durations (CDN manifest lacks durations per CLAUDE.md).
# Pull from the API which is the canonical source:
curl -fsSL "$AUDIO_MANIFEST_URL" -o output/audio-manifest-cdn.json

# ─── 5. Download per-paper audio MP3s ───
# The renderer supports --audio-dir pointing at a flat layout of
# tts-1-hd-nova-{globalId}.mp3 files (per the urantia-data-sources convention).
# We'll fetch them lazily in the manifest phase, OR preload in one burst.

./target/release/urantia-render download --papers "$PAPER_RANGE" || {
    echo "download command failed or unavailable; continuing, render will try CDN fetch"
}

# ─── 6. Build manifests ───
./target/release/urantia-render manifest --papers "$PAPER_RANGE"

# ─── 7. Render all papers in parallel ───
export URANTIA_RENDER_ENCODER=libx264
export URANTIA_RENDER_THREADS="$THREADS_PER_FFMPEG"

./target/release/urantia-render render \
    --papers "$PAPER_RANGE" \
    --concurrency "$CONCURRENCY" \
    --skip-existing

# ─── 8. Generate metadata JSON (uses api.urantia.dev) ───
./target/release/urantia-render metadata --papers "$PAPER_RANGE" || true

# ─── 9. Upload outputs to S3 ───
aws s3 sync output/videos/   "s3://$S3_BUCKET/$S3_PREFIX/videos/"   --only-show-errors
aws s3 sync output/metadata/ "s3://$S3_BUCKET/$S3_PREFIX/metadata/" --only-show-errors

echo "[$(date -u)] batch complete: s3://$S3_BUCKET/$S3_PREFIX/"

# ─── 10. Self-terminate to stop spot billing ───
if [ "${AUTO_TERMINATE:-0}" = "1" ]; then
    REGION=$(curl -s http://169.254.169.254/latest/meta-data/placement/region)
    INSTANCE_ID=$(curl -s http://169.254.169.254/latest/meta-data/instance-id)
    aws ec2 terminate-instances --region "$REGION" --instance-ids "$INSTANCE_ID"
fi
