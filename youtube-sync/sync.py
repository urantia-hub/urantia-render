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
YouTube bulk-update for the UrantiaHub channel.

Reads enriched metadata JSON from `../output/metadata/{N}.json` (produced by
`urantia-render metadata`) and pushes title/description/tags to the matching
already-uploaded YouTube video.

Usage:
    # One-time: map paper IDs to YouTube video IDs (writes mapping.json).
    ./sync.py list

    # Preview updates without writing to YouTube.
    ./sync.py push --dry-run

    # Actually push updates.
    ./sync.py push

    # Push a single paper (e.g. to test a specific title).
    ./sync.py push --paper 1

    # Also upload new thumbnails.
    ./sync.py push --thumbnails

Auth:
    1. Create a Google Cloud project + enable "YouTube Data API v3".
    2. Create an OAuth 2.0 Client ID (type: Desktop app).
    3. Download the JSON → save as youtube-sync/client_secret.json.
    4. First run opens a browser; token is cached in youtube-sync/token.json.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path

from google.auth.transport.requests import Request
from google.oauth2.credentials import Credentials
from google_auth_oauthlib.flow import InstalledAppFlow
from googleapiclient.discovery import build
from googleapiclient.http import MediaFileUpload

SCOPES = ["https://www.googleapis.com/auth/youtube"]
HERE = Path(__file__).resolve().parent
OUTPUT_DIR = HERE.parent / "output"
METADATA_DIR = OUTPUT_DIR / "metadata"
THUMBS_DIR = OUTPUT_DIR / "thumbnails"
VIDEOS_DIR = OUTPUT_DIR / "videos"
CLIENT_SECRET = HERE / "client_secret.json"
TOKEN_FILE = HERE / "token.json"
MAPPING_FILE = HERE / "mapping.json"
PLAYLISTS_FILE = HERE / "playlists.json"
UPLOAD_STATE_FILE = HERE / "upload-state.json"

# Playlist config. Keys are stable IDs we use inside the script; values are
# the actual YouTube playlist titles + which paper IDs belong.
PLAYLISTS = {
    "all": {
        "title": "The Urantia Papers — Full Audio",
        "description": "All 197 Urantia Papers, narrated with AI voice and synced text. Read along with every paper of The Urantia Papers.",
    },
    "part-1": {
        "title": "Part I — The Central and Superuniverses",
        "description": "Papers 1–31 of The Urantia Papers. The nature of God, the Paradise Trinity, Havona, and the central universe.",
    },
    "part-2": {
        "title": "Part II — The Local Universe",
        "description": "Papers 32–56 of The Urantia Papers. The evolution and administration of local universes, including our own Nebadon.",
    },
    "part-3": {
        "title": "Part III — The History of Urantia",
        "description": "Papers 57–119 of The Urantia Papers. The history of our planet from its formation through spiritual epochs.",
    },
    "part-4": {
        "title": "Part IV — The Life and Teachings of Jesus",
        "description": "Papers 120–196 of The Urantia Papers. The life, teachings, and mission of Jesus of Nazareth.",
    },
}


def playlist_keys_for_paper(paper_id: str) -> list[str]:
    """Return the playlist keys a paper should be added to, by paper id."""
    p = int(paper_id)
    keys = ["all"]
    if 1 <= p <= 31:
        keys.append("part-1")
    elif 32 <= p <= 56:
        keys.append("part-2")
    elif 57 <= p <= 119:
        keys.append("part-3")
    elif 120 <= p <= 196:
        keys.append("part-4")
    # Paper 0 (Foreword) → "all" only.
    return keys

# Matches "Paper 1", "Paper 42", "Foreword" in existing video titles.
TITLE_RE = re.compile(r"(Paper\s+)(\d{1,3})\b", re.IGNORECASE)
FOREWORD_RE = re.compile(r"\bForeword\b", re.IGNORECASE)


def get_service():
    """Build an authenticated YouTube Data API v3 client."""
    if not CLIENT_SECRET.exists():
        sys.exit(
            f"missing {CLIENT_SECRET}. see auth instructions at the top of sync.py"
        )

    creds = None
    if TOKEN_FILE.exists():
        creds = Credentials.from_authorized_user_file(str(TOKEN_FILE), SCOPES)

    if not creds or not creds.valid:
        if creds and creds.expired and creds.refresh_token:
            creds.refresh(Request())
        else:
            flow = InstalledAppFlow.from_client_secrets_file(str(CLIENT_SECRET), SCOPES)
            creds = flow.run_local_server(port=0)
        TOKEN_FILE.write_text(creds.to_json())
        os.chmod(TOKEN_FILE, 0o600)

    return build("youtube", "v3", credentials=creds)


def list_uploads(yt) -> list[dict]:
    """Page through every video on the authenticated channel."""
    # Find the uploads playlist for the authenticated channel.
    channels = yt.channels().list(part="contentDetails", mine=True).execute()
    items = channels.get("items", [])
    if not items:
        sys.exit("no channel found for authenticated user")
    uploads_playlist = items[0]["contentDetails"]["relatedPlaylists"]["uploads"]

    videos: list[dict] = []
    token = None
    while True:
        resp = (
            yt.playlistItems()
            .list(
                part="snippet,contentDetails",
                playlistId=uploads_playlist,
                maxResults=50,
                pageToken=token,
            )
            .execute()
        )
        for it in resp.get("items", []):
            videos.append(
                {
                    "videoId": it["contentDetails"]["videoId"],
                    "title": it["snippet"]["title"],
                    "publishedAt": it["snippet"].get("publishedAt"),
                }
            )
        token = resp.get("nextPageToken")
        if not token:
            break
    return videos


def paper_id_from_title(title: str) -> str | None:
    """Extract the paper ID from a video title, e.g. '...Paper 1...' -> '1'."""
    m = TITLE_RE.search(title)
    if m:
        return m.group(2).lstrip("0") or "0"
    if FOREWORD_RE.search(title):
        return "0"
    return None


def cmd_list(args):
    """Fetch the channel's videos and persist paperId -> videoId mapping."""
    yt = get_service()
    videos = list_uploads(yt)
    mapping: dict[str, str] = {}
    unresolved: list[dict] = []

    for v in videos:
        pid = paper_id_from_title(v["title"])
        if pid is not None:
            # If multiple videos map to the same paper (re-uploads), keep the
            # oldest — YouTube shows URL history for that one, and newer clones
            # are likely accidents.
            if pid not in mapping or (
                v.get("publishedAt") and v["publishedAt"] < mapping.get(f"_meta_{pid}", "9999")
            ):
                mapping[pid] = v["videoId"]
                mapping[f"_meta_{pid}"] = v.get("publishedAt") or ""
        else:
            unresolved.append(v)

    # Strip meta keys before serializing.
    clean = {k: v for k, v in mapping.items() if not k.startswith("_meta_")}

    MAPPING_FILE.write_text(json.dumps(clean, indent=2, sort_keys=True))
    print(f"mapped {len(clean)} videos -> {MAPPING_FILE.relative_to(HERE.parent)}")
    if unresolved:
        print(f"\n{len(unresolved)} videos did NOT match a paper id:")
        for v in unresolved[:20]:
            print(f"  {v['videoId']}  {v['title']}")
        if len(unresolved) > 20:
            print(f"  ... and {len(unresolved) - 20} more")


def cmd_push(args):
    """Push metadata (and optionally thumbnails) to YouTube."""
    if not MAPPING_FILE.exists():
        sys.exit(f"missing {MAPPING_FILE}. run `./sync.py list` first.")
    mapping = json.loads(MAPPING_FILE.read_text())

    yt = get_service()

    if args.paper:
        paper_ids = [args.paper]
    else:
        paper_ids = sorted(
            [k for k in mapping.keys() if not k.startswith("_")], key=lambda s: int(s)
        )

    updated = 0
    skipped = 0

    for pid in paper_ids:
        video_id = mapping.get(pid)
        if not video_id:
            print(f"  paper {pid}: no video mapping, skipping")
            skipped += 1
            continue

        meta_path = METADATA_DIR / f"{pid}.json"
        if not meta_path.exists():
            print(f"  paper {pid}: no metadata json at {meta_path}, skipping")
            skipped += 1
            continue

        meta = json.loads(meta_path.read_text())

        # Truncate description to YouTube's 5000-char limit.
        description = meta["description"]
        if len(description) > 5000:
            description = description[:4997] + "..."

        # Cap tags at ~500 chars total (YouTube limit).
        tags = meta["tags"]
        while sum(len(t) + 2 for t in tags) > 500 and tags:
            tags = tags[:-1]

        # If --title-and-tags-only, preserve the existing YouTube description
        # (keeps chapter timestamps aligned with the currently-uploaded video
        # audio, which may differ from the freshly regenerated manifest).
        if args.title_and_tags_only:
            current = yt.videos().list(part="snippet", id=video_id).execute()
            items = current.get("items", [])
            if not items:
                print(f"  paper {pid}: video {video_id} not found on YouTube")
                skipped += 1
                continue
            description = items[0]["snippet"].get("description", "")

        body = {
            "id": video_id,
            "snippet": {
                "title": meta["title"],
                "description": description,
                "tags": tags,
                # Education = 27. Keep category pinned per the upload guide.
                "categoryId": "27",
            },
        }

        if args.thumbnails_only:
            thumb = THUMBS_DIR / f"thumbnail-{pid}.png"
            if not thumb.exists():
                print(f"  paper {pid}: no thumbnail at {thumb}, skipping")
                skipped += 1
                continue
            if args.dry_run:
                print(f"  [dry-run] paper {pid} ({video_id}): would upload {thumb.name}")
                updated += 1
                continue
            try:
                yt.thumbnails().set(
                    videoId=video_id,
                    media_body=MediaFileUpload(str(thumb), mimetype="image/png"),
                ).execute()
                print(f"  paper {pid}: thumbnail uploaded ({thumb.name})")
                updated += 1
            except Exception as e:
                print(f"  paper {pid}: thumbnail FAILED — {e}")
                skipped += 1
            continue

        if args.dry_run:
            mode = "title+tags" if args.title_and_tags_only else "title+tags+description"
            print(
                f"  [dry-run] paper {pid} ({video_id}): would update {mode}"
                f"\n    title: {meta['title']}"
                f"\n    tags:  {', '.join(tags)}"
            )
            updated += 1
            continue

        try:
            yt.videos().update(part="snippet", body=body).execute()
            print(f"  paper {pid}: updated {video_id}")
            updated += 1

            if args.thumbnails:
                thumb = THUMBS_DIR / f"thumbnail-{pid}.png"
                if thumb.exists():
                    yt.thumbnails().set(
                        videoId=video_id,
                        media_body=MediaFileUpload(str(thumb), mimetype="image/png"),
                    ).execute()
                    print(f"    thumbnail uploaded: {thumb.name}")
                else:
                    print(f"    no thumbnail at {thumb}")
        except Exception as e:
            print(f"  paper {pid}: FAILED — {e}")
            skipped += 1

    print(f"\n{'DRY-RUN ' if args.dry_run else ''}done: {updated} updated, {skipped} skipped")


def cmd_diff(args):
    """Show before/after for a specific paper — current YouTube metadata vs
    the freshly generated JSON. Read-only, costs 1 unit per paper."""
    if not MAPPING_FILE.exists():
        sys.exit(f"missing {MAPPING_FILE}. run `./sync.py list` first.")
    mapping = json.loads(MAPPING_FILE.read_text())

    paper_ids = [args.paper] if args.paper else sorted(mapping.keys(), key=lambda s: int(s))
    if not args.paper:
        # Default to a representative set so `--all` isn't the default.
        paper_ids = [p for p in ["0", "1", "42", "100", "118", "196"] if p in mapping]

    yt = get_service()

    for pid in paper_ids:
        video_id = mapping.get(pid)
        if not video_id:
            print(f"\n=== paper {pid}: no video mapping ===")
            continue
        meta_path = METADATA_DIR / f"{pid}.json"
        if not meta_path.exists():
            print(f"\n=== paper {pid}: no metadata json ===")
            continue

        new_meta = json.loads(meta_path.read_text())
        current = yt.videos().list(part="snippet", id=video_id).execute()
        items = current.get("items", [])
        if not items:
            print(f"\n=== paper {pid}: video {video_id} not found on YouTube ===")
            continue
        snip = items[0]["snippet"]

        print(f"\n=== paper {pid} — video {video_id} ===")
        print(f"--- TITLE ---")
        print(f"  current: {snip.get('title')}")
        print(f"  new:     {new_meta['title']}")

        print(f"--- TAGS ---")
        cur_tags = snip.get("tags", [])
        print(f"  current ({len(cur_tags)}): {', '.join(cur_tags) if cur_tags else '(none)'}")
        print(f"  new ({len(new_meta['tags'])}):     {', '.join(new_meta['tags'])}")

        print(f"--- DESCRIPTION (first 400 chars) ---")
        cur_desc = (snip.get("description") or "")[:400].replace("\n", "\\n ")
        new_desc = new_meta["description"][:400].replace("\n", "\\n ")
        print(f"  current: {cur_desc}...")
        print(f"  new:     {new_desc}...")


def cmd_playlists(args):
    """Create the 5 playlists if missing, persist their IDs to playlists.json.

    Idempotent: matches on title exactly. If a playlist with the same title
    already exists, reuses it instead of creating a duplicate.
    """
    yt = get_service()

    # Load existing IDs if we have them, then fill in the blanks.
    existing: dict[str, str] = {}
    if PLAYLISTS_FILE.exists():
        existing = json.loads(PLAYLISTS_FILE.read_text())

    # Fetch every playlist on the channel so we can match by title.
    channel_playlists: dict[str, str] = {}
    token = None
    while True:
        resp = (
            yt.playlists()
            .list(part="snippet", mine=True, maxResults=50, pageToken=token)
            .execute()
        )
        for it in resp.get("items", []):
            channel_playlists[it["snippet"]["title"]] = it["id"]
        token = resp.get("nextPageToken")
        if not token:
            break

    out: dict[str, str] = {}
    for key, cfg in PLAYLISTS.items():
        title = cfg["title"]
        if title in channel_playlists:
            out[key] = channel_playlists[title]
            print(f"  {key}: exists — {out[key]} ({title})")
            continue
        if args.dry_run:
            print(f"  [dry-run] would create playlist '{title}' ({key})")
            continue
        created = (
            yt.playlists()
            .insert(
                part="snippet,status",
                body={
                    "snippet": {"title": title, "description": cfg["description"]},
                    "status": {"privacyStatus": "public"},
                },
            )
            .execute()
        )
        out[key] = created["id"]
        print(f"  {key}: created — {out[key]} ({title})")

    if not args.dry_run:
        PLAYLISTS_FILE.write_text(json.dumps(out, indent=2))
        print(f"\n→ {PLAYLISTS_FILE.relative_to(HERE.parent)}")


def cmd_upload(args):
    """Upload freshly-rendered 4K videos to YouTube.

    For each paper in range:
      1. videos.insert the MP4 (1600 quota units).
      2. thumbnails.set the 4K thumbnail (50 units).
      3. playlistItems.insert to "All Papers" + the relevant Part playlist
         (50 units × 2 = 100 units).

    Per-paper quota cost: ~1750 units. Daily default is 10,000 — plan for
    ~5 uploads/day unless you've requested a quota increase. State is
    persisted to upload-state.json so re-runs resume cleanly.
    """
    if not PLAYLISTS_FILE.exists():
        sys.exit(f"missing {PLAYLISTS_FILE}. run `./sync.py playlists` first.")
    playlists = json.loads(PLAYLISTS_FILE.read_text())

    state: dict[str, str] = {}
    if UPLOAD_STATE_FILE.exists():
        state = json.loads(UPLOAD_STATE_FILE.read_text())

    paper_ids = _expand_range(args.papers)
    yt = get_service()

    uploaded = 0
    skipped = 0
    quota_used = 0

    for pid in paper_ids:
        if pid in state and not args.force:
            print(f"  paper {pid}: already uploaded as {state[pid]}, skipping")
            skipped += 1
            continue

        video_path = VIDEOS_DIR / f"tts-1-hd-nova-{pid}.mp4"
        meta_path = METADATA_DIR / f"{pid}.json"
        thumb_path = THUMBS_DIR / f"thumbnail-{pid}.png"

        if not video_path.exists():
            print(f"  paper {pid}: missing {video_path}, skipping")
            skipped += 1
            continue
        if not meta_path.exists():
            print(f"  paper {pid}: missing {meta_path}, skipping")
            skipped += 1
            continue

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

        if args.dry_run:
            plists = [playlists.get(k, "?") for k in playlist_keys_for_paper(pid)]
            print(f"  [dry-run] paper {pid}:")
            print(f"    title:     {meta['title']}")
            print(f"    video:     {video_path.name} ({video_path.stat().st_size / 1024 / 1024:.0f} MB)")
            print(f"    thumbnail: {thumb_path.name if thumb_path.exists() else '(missing)'}")
            print(f"    playlists: {' + '.join(plists)}")
            uploaded += 1
            continue

        try:
            media = MediaFileUpload(
                str(video_path), chunksize=8 * 1024 * 1024, resumable=True, mimetype="video/mp4"
            )
            req = yt.videos().insert(part="snippet,status", body=body, media_body=media)
            response = None
            while response is None:
                status, response = req.next_chunk()
                if status:
                    pct = int(status.progress() * 100)
                    print(f"\r  paper {pid}: uploading {pct}%", end="", flush=True)
            video_id = response["id"]
            print(f"\r  paper {pid}: uploaded as {video_id}")
            quota_used += 1600

            if thumb_path.exists():
                yt.thumbnails().set(
                    videoId=video_id,
                    media_body=MediaFileUpload(str(thumb_path), mimetype="image/png"),
                ).execute()
                print(f"    thumbnail: {thumb_path.name}")
                quota_used += 50

            for key in playlist_keys_for_paper(pid):
                pl_id = playlists.get(key)
                if not pl_id:
                    print(f"    WARN: playlist key '{key}' not in playlists.json")
                    continue
                yt.playlistItems().insert(
                    part="snippet",
                    body={
                        "snippet": {
                            "playlistId": pl_id,
                            "resourceId": {"kind": "youtube#video", "videoId": video_id},
                        }
                    },
                ).execute()
                print(f"    playlist: {PLAYLISTS[key]['title']}")
                quota_used += 50

            state[pid] = video_id
            UPLOAD_STATE_FILE.write_text(json.dumps(state, indent=2, sort_keys=True))
            uploaded += 1
            print(f"    (quota used so far: ~{quota_used} units)")

        except Exception as e:
            print(f"\n  paper {pid}: FAILED — {e}")
            # Persist whatever we have so resumes don't lose ground.
            if state:
                UPLOAD_STATE_FILE.write_text(json.dumps(state, indent=2, sort_keys=True))
            if "quotaExceeded" in str(e):
                print("  quota exhausted — stop and retry tomorrow with --resume")
                break
            skipped += 1

    print(f"\n{'DRY-RUN ' if args.dry_run else ''}done: {uploaded} uploaded, {skipped} skipped")
    print(f"approximate quota used: {quota_used} units (10,000/day default)")


def cmd_delete(args):
    """Delete videos by paper id (from mapping.json). Requires --yes to actually delete.

    Intended for the 'clean slate' workflow: delete the old 1080p uploads
    before bulk-uploading the new 4K versions. Quota cost: 50 units per
    delete.
    """
    if not MAPPING_FILE.exists():
        sys.exit(f"missing {MAPPING_FILE}. run `./sync.py list` first.")
    mapping = json.loads(MAPPING_FILE.read_text())

    paper_ids = _expand_range(args.papers) if args.papers else sorted(
        [k for k in mapping.keys() if not k.startswith("_")], key=lambda s: int(s)
    )

    targets = [(pid, mapping[pid]) for pid in paper_ids if pid in mapping]
    if not targets:
        sys.exit("no matching videos to delete")

    print(f"will delete {len(targets)} videos:")
    for pid, vid in targets[:20]:
        print(f"  paper {pid}: {vid}")
    if len(targets) > 20:
        print(f"  ... and {len(targets) - 20} more")

    if not args.yes:
        print("\nrerun with --yes to actually delete. quota: 50 units per delete.")
        return

    yt = get_service()
    deleted = 0
    for pid, vid in targets:
        try:
            yt.videos().delete(id=vid).execute()
            print(f"  deleted paper {pid}: {vid}")
            deleted += 1
        except Exception as e:
            print(f"  paper {pid}: FAILED — {e}")
            if "quotaExceeded" in str(e):
                break

    print(f"\ndeleted {deleted} of {len(targets)}")


def _expand_range(spec: str | None) -> list[str]:
    """Parse a '1,3,5' or '0-196' spec into a sorted list of string ids."""
    if not spec:
        return []
    out: set[int] = set()
    for part in spec.split(","):
        part = part.strip()
        if "-" in part:
            a, b = part.split("-", 1)
            out.update(range(int(a), int(b) + 1))
        elif part:
            out.add(int(part))
    return [str(x) for x in sorted(out)]


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = p.add_subparsers(dest="cmd", required=True)

    sp_list = sub.add_parser("list", help="discover channel videos and save mapping.json")
    sp_list.set_defaults(func=cmd_list)

    sp_diff = sub.add_parser("diff", help="show current-vs-proposed metadata for spot-check")
    sp_diff.add_argument("--paper", help="single paper id (defaults to a representative set)")
    sp_diff.set_defaults(func=cmd_diff)

    sp_push = sub.add_parser("push", help="push metadata (+ thumbnails) to YouTube")
    sp_push.add_argument("--dry-run", action="store_true", help="preview without writing")
    sp_push.add_argument("--paper", help="push a single paper id, e.g. --paper 1")
    sp_push.add_argument(
        "--thumbnails", action="store_true", help="also upload thumbnail-{id}.png"
    )
    sp_push.add_argument(
        "--title-and-tags-only",
        action="store_true",
        help=(
            "update title + tags only; keep the existing YouTube description "
            "(useful when the uploaded video's audio timing differs from the "
            "freshly-generated chapter timestamps)"
        ),
    )
    sp_push.add_argument(
        "--thumbnails-only",
        action="store_true",
        help=(
            "only upload thumbnails; skip the videos.update call entirely. "
            "Useful for a second-day pass after quota reset."
        ),
    )
    sp_push.set_defaults(func=cmd_push)

    sp_playlists = sub.add_parser(
        "playlists", help="create 5 channel playlists (idempotent) and save IDs"
    )
    sp_playlists.add_argument("--dry-run", action="store_true")
    sp_playlists.set_defaults(func=cmd_playlists)

    sp_upload = sub.add_parser(
        "upload", help="upload freshly-rendered 4K videos + thumbnails + playlist assignment"
    )
    sp_upload.add_argument(
        "--papers", required=True, help="paper range, e.g. '1' or '0-196' or '1,3,5'"
    )
    sp_upload.add_argument("--dry-run", action="store_true")
    sp_upload.add_argument(
        "--force", action="store_true", help="re-upload even if paper is in upload-state.json"
    )
    sp_upload.set_defaults(func=cmd_upload)

    sp_delete = sub.add_parser(
        "delete", help="delete videos by paper id (from mapping.json)"
    )
    sp_delete.add_argument(
        "--papers", help="paper range (default: all mapped)"
    )
    sp_delete.add_argument("--yes", action="store_true", help="confirm deletion")
    sp_delete.set_defaults(func=cmd_delete)

    args = p.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
