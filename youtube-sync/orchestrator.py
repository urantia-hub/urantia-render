#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "google-api-python-client>=2.120",
#     "google-auth-oauthlib>=1.2",
#     "google-auth-httplib2>=0.2",
# ]
# ///
"""
Background orchestrator for the UrantiaHub uploader.

Loops forever. Each cycle:
  1. Check the PAUSED marker; if present, sleep and retry.
  2. Pick the next paper not yet uploaded (in ID order, 0 through 196).
  3. Ensure assets (MP4 + metadata + thumbnail) exist; render if missing.
  4. If today's quota has room: upload via YouTube Data API. Persist progress.
  5. If quota is exhausted: switch to render-ahead mode — walk forward through
     remaining papers and render any that are missing MP4/metadata/thumbnail,
     so the queue is pre-warmed for tomorrow's uploads. Only sleep when every
     remaining paper is already rendered.

Run this directly for foreground debugging:
    ./orchestrator.py

Intended production use: via nohup (see bin/urantia-uploader).
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path

# Reuse sync.py's helpers — same auth, same playlist logic.
sys.path.insert(0, str(Path(__file__).parent))
from sync import (  # noqa: E402
    HERE, METADATA_DIR, MAPPING_FILE, PLAYLISTS_FILE, UPLOAD_STATE_FILE,
    THUMBS_DIR, VIDEOS_DIR, PLAYLISTS, get_service, playlist_keys_for_paper,
    list_uploads, paper_id_from_title,
)
from googleapiclient.http import MediaFileUpload  # noqa: E402

# ─── Paths ───
ROOT = HERE.parent  # urantia-render/
RENDER_BIN = ROOT / "target" / "release" / "urantia-render"
PAUSED_MARKER = HERE / "PAUSED"
QUOTA_LOG = HERE / "quota-log.json"
LOG_FILE = HERE / "orchestrator.log"

# ─── Tuning ───
PAPER_IDS = [str(i) for i in range(0, 197)]  # 0..196
PER_UPLOAD_COST = 1750  # videos.insert 1600 + thumbnails.set 50 + 2x playlistItems.insert 100
CHANNEL_REFRESH_COST = 5  # channels.list 1 + ~4 playlistItems.list pages
DAILY_QUOTA_BUDGET = 8500  # leave a 1500-unit buffer under the 10k hard limit
SLEEP_AFTER_UPLOAD_SEC = 600  # 10 min — spread uploads through the day
SLEEP_WHEN_PAUSED_SEC = 300  # 5 min
SLEEP_ON_ERROR_SEC = 900  # 15 min — back off on transient errors


def log(msg: str) -> None:
    """Append a timestamped line to the log and stdout."""
    line = f"[{datetime.now(timezone.utc).isoformat(timespec='seconds')}] {msg}"
    print(line, flush=True)
    try:
        with LOG_FILE.open("a") as f:
            f.write(line + "\n")
    except OSError:
        pass


def today_pt() -> str:
    """YYYY-MM-DD in America/Los_Angeles (matches YouTube's quota reset clock)."""
    # YouTube resets at midnight Pacific Time. Approximate with UTC-8 (good
    # enough — we only care about day boundaries, not DST edge cases).
    pt = datetime.now(timezone.utc) - timedelta(hours=8)
    return pt.strftime("%Y-%m-%d")


def seconds_until_next_pt_reset() -> int:
    """Seconds until 00:00 PT tomorrow — when the quota resets."""
    now_pt = datetime.now(timezone.utc) - timedelta(hours=8)
    tomorrow_midnight_pt = (now_pt + timedelta(days=1)).replace(
        hour=0, minute=0, second=0, microsecond=0
    )
    return max(60, int((tomorrow_midnight_pt - now_pt).total_seconds()))


def load_quota_log() -> dict[str, int]:
    if QUOTA_LOG.exists():
        try:
            return json.loads(QUOTA_LOG.read_text())
        except json.JSONDecodeError:
            return {}
    return {}


def quota_remaining_today() -> int:
    used = load_quota_log().get(today_pt(), 0)
    return max(0, DAILY_QUOTA_BUDGET - used)


def record_quota_usage(units: int) -> None:
    log_ = load_quota_log()
    day = today_pt()
    log_[day] = log_.get(day, 0) + units
    # Prune entries older than 30 days to keep the file small.
    cutoff = (datetime.now(timezone.utc) - timedelta(days=30)).strftime("%Y-%m-%d")
    log_ = {d: n for d, n in log_.items() if d >= cutoff}
    QUOTA_LOG.write_text(json.dumps(log_, indent=2, sort_keys=True))


def load_upload_state() -> dict[str, str]:
    if UPLOAD_STATE_FILE.exists():
        return json.loads(UPLOAD_STATE_FILE.read_text())
    return {}


def save_upload_state(state: dict[str, str]) -> None:
    UPLOAD_STATE_FILE.write_text(json.dumps(state, indent=2, sort_keys=True))


def next_paper_id(done: dict[str, str]) -> str | None:
    for pid in PAPER_IDS:
        if pid not in done:
            return pid
    return None


def run_cargo_subcommand(subcmd: str, paper_id: str) -> bool:
    """Invoke the rust binary for one paper. Returns True on success."""
    if not RENDER_BIN.exists():
        log(f"ERROR: render binary missing at {RENDER_BIN}. Run `cargo build --release` first.")
        return False
    cmd = [str(RENDER_BIN), subcmd, "--papers", paper_id]
    log(f"→ urantia-render {subcmd} --papers {paper_id}")
    try:
        result = subprocess.run(cmd, cwd=ROOT, check=False, capture_output=True, text=True, timeout=3 * 60 * 60)
    except subprocess.TimeoutExpired:
        log(f"ERROR: `{subcmd}` timed out for paper {paper_id}")
        return False
    if result.returncode != 0:
        log(f"ERROR: `{subcmd}` exited {result.returncode}. stderr tail:\n{result.stderr[-500:]}")
        return False
    return True


def ensure_paper_assets(pid: str) -> bool:
    """Render MP4, metadata, and thumbnail for a paper if missing. Returns True on success."""
    video_path = VIDEOS_DIR / f"tts-1-hd-nova-{pid}.mp4"
    meta_path = METADATA_DIR / f"{pid}.json"
    thumb_path = THUMBS_DIR / f"thumbnail-{pid}.png"

    if not video_path.exists():
        log(f"paper {pid}: rendering MP4 (this takes ~45-50 min on M1)")
        if not run_cargo_subcommand("render", pid):
            return False
    if not meta_path.exists():
        log(f"paper {pid}: generating metadata")
        if not run_cargo_subcommand("metadata", pid):
            return False
    if not thumb_path.exists():
        log(f"paper {pid}: generating thumbnail")
        if not run_cargo_subcommand("thumbnail", pid):
            return False

    return video_path.exists() and meta_path.exists() and thumb_path.exists()


def upload_paper(pid: str, yt) -> str | None:
    """Upload one paper via the YouTube API. Returns the new videoId on success."""
    video_path = VIDEOS_DIR / f"tts-1-hd-nova-{pid}.mp4"
    meta_path = METADATA_DIR / f"{pid}.json"
    thumb_path = THUMBS_DIR / f"thumbnail-{pid}.png"

    meta = json.loads(meta_path.read_text())

    description = meta["description"]
    if len(description) > 5000:
        description = description[:4997] + "..."
    tags = meta["tags"]
    while sum(len(t) + 2 for t in tags) > 500 and tags:
        tags = tags[:-1]

    body = {
        "snippet": {
            "title": meta["title"],
            "description": description,
            "tags": tags,
            "categoryId": "27",  # Education
        },
        "status": {
            "privacyStatus": "public",
            "madeForKids": False,
            "selfDeclaredMadeForKids": False,
        },
    }

    log(f"paper {pid}: uploading {video_path.name} ({video_path.stat().st_size / 1024 / 1024:.0f} MB)")
    media = MediaFileUpload(str(video_path), chunksize=8 * 1024 * 1024, resumable=True, mimetype="video/mp4")
    req = yt.videos().insert(part="snippet,status", body=body, media_body=media)
    response = None
    last_pct = 0
    while response is None:
        status, response = req.next_chunk()
        if status and int(status.progress() * 100) >= last_pct + 10:
            last_pct = int(status.progress() * 100)
            log(f"  upload {last_pct}%")
    video_id = response["id"]
    log(f"paper {pid}: uploaded as {video_id}")

    yt.thumbnails().set(
        videoId=video_id,
        media_body=MediaFileUpload(str(thumb_path), mimetype="image/png"),
    ).execute()
    log(f"paper {pid}: thumbnail set")

    playlists = json.loads(PLAYLISTS_FILE.read_text())
    for key in playlist_keys_for_paper(pid):
        pl_id = playlists.get(key)
        if not pl_id:
            log(f"  WARN: playlist key '{key}' not in playlists.json")
            continue
        yt.playlistItems().insert(
            part="snippet",
            body={"snippet": {"playlistId": pl_id, "resourceId": {"kind": "youtube#video", "videoId": video_id}}},
        ).execute()
        log(f"  added to playlist: {PLAYLISTS[key]['title']}")

    return video_id


def refresh_upload_state_from_channel(yt, done: dict[str, str]) -> dict[str, str]:
    """Query the channel and merge any discovered paper-N videos into upload-state.json.

    Protects against duplicate uploads when the user manually adds a paper via
    YouTube Studio (to bypass API quota) and forgets to run `mark-done`. Called
    before each upload cycle. Cheap — ~5 quota units per refresh.

    Only adds missing entries. If the channel has paper N as videoId Y but
    local state already has paper N as videoId X, keep X and log a warning —
    the user can override with `mark-done` if they really meant to swap.
    """
    videos = list_uploads(yt)
    record_quota_usage(CHANNEL_REFRESH_COST)
    merged = 0
    for v in videos:
        pid = paper_id_from_title(v["title"])
        if pid is None:
            continue
        if pid not in done:
            done[pid] = v["videoId"]
            log(f"channel already has paper {pid} as {v['videoId']} — merged into upload-state")
            merged += 1
        elif done[pid] != v["videoId"]:
            log(
                f"WARN: channel has paper {pid} as {v['videoId']} but local state "
                f"says {done[pid]}. Keeping local. Run `mark-done {pid} {v['videoId']}` to swap."
            )
    if merged:
        save_upload_state(done)
    return done


def assets_exist(pid: str) -> bool:
    return (
        (VIDEOS_DIR / f"tts-1-hd-nova-{pid}.mp4").exists()
        and (METADATA_DIR / f"{pid}.json").exists()
        and (THUMBS_DIR / f"thumbnail-{pid}.png").exists()
    )


def render_ahead_or_sleep(done: dict[str, str]) -> int:
    """Find the next not-yet-uploaded paper that's missing assets and render them.
    Returns 60s if we made progress (loop back quickly in case quota reset),
    or seconds_until_next_pt_reset() if everything remaining is pre-rendered.

    Rationale: rendering is local and free of API quota. Pre-warming assets
    means (a) when quota resets we upload instantly instead of waiting ~50min
    per render, and (b) if the user wants to drag-and-drop to YouTube Studio
    during the quota window, the MP4/thumbnail/metadata are already on disk.
    """
    for pid in PAPER_IDS:
        if pid in done:
            continue
        if assets_exist(pid):
            continue
        log(f"render-ahead: preparing paper {pid}")
        if not ensure_paper_assets(pid):
            log(f"render-ahead: paper {pid} failed; backing off")
            return SLEEP_ON_ERROR_SEC
        # Rendered one paper (~45min elapsed) — loop back so we re-check
        # the quota window in case PT midnight passed while we were busy.
        return 60

    wait = seconds_until_next_pt_reset()
    log(f"all remaining papers pre-rendered; waiting {wait / 3600:.1f}h for quota reset")
    return wait


def one_iteration() -> int:
    """Do one unit of work. Return seconds to sleep before the next iteration."""
    if PAUSED_MARKER.exists():
        log("paused (marker file present); sleeping")
        return SLEEP_WHEN_PAUSED_SEC

    if not PLAYLISTS_FILE.exists():
        log("ERROR: youtube-sync/playlists.json missing. Run `./sync.py playlists` first.")
        return SLEEP_ON_ERROR_SEC

    done = load_upload_state()
    pid = next_paper_id(done)
    if pid is None:
        log("all 197 papers uploaded — nothing to do. Sleeping 1 hour before next check.")
        return 3600

    remaining = quota_remaining_today()
    if remaining < PER_UPLOAD_COST:
        log(f"quota exhausted today ({remaining} units left); entering render-ahead mode")
        return render_ahead_or_sleep(done)

    # About to upload — first, refresh state from the channel so we don't
    # re-upload any paper the user manually added via YouTube Studio.
    yt = get_service()
    try:
        done = refresh_upload_state_from_channel(yt, done)
    except Exception as e:
        log(f"WARN: channel refresh failed ({e!r}); proceeding with local state")

    pid = next_paper_id(done)
    if pid is None:
        log("all 197 papers uploaded — nothing to do. Sleeping 1 hour before next check.")
        return 3600

    if not ensure_paper_assets(pid):
        log(f"paper {pid}: asset generation failed; backing off")
        return SLEEP_ON_ERROR_SEC

    try:
        video_id = upload_paper(pid, yt)
    except Exception as e:
        msg = str(e)
        if "quotaExceeded" in msg:
            log("quotaExceeded from API; entering render-ahead mode")
            record_quota_usage(PER_UPLOAD_COST)
            return render_ahead_or_sleep(done)
        log(f"paper {pid}: upload FAILED — {e}")
        return SLEEP_ON_ERROR_SEC

    if video_id:
        done[pid] = video_id
        save_upload_state(done)
        record_quota_usage(PER_UPLOAD_COST)
        uploaded = len(done)
        log(f"progress: {uploaded}/197 papers uploaded ({uploaded / 197 * 100:.1f}%)")

    return SLEEP_AFTER_UPLOAD_SEC


def main() -> None:
    log(f"orchestrator starting (pid {os.getpid()}, root {ROOT})")
    while True:
        try:
            sleep_for = one_iteration()
        except KeyboardInterrupt:
            log("interrupted, exiting")
            return
        except Exception as e:
            log(f"unexpected error in iteration: {e!r}; backing off")
            sleep_for = SLEEP_ON_ERROR_SEC
        log(f"sleeping {sleep_for}s ({sleep_for / 60:.1f} min)")
        time.sleep(sleep_for)


if __name__ == "__main__":
    main()
