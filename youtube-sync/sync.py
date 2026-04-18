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
CLIENT_SECRET = HERE / "client_secret.json"
TOKEN_FILE = HERE / "token.json"
MAPPING_FILE = HERE / "mapping.json"

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
        paper_ids = sorted(mapping.keys(), key=lambda s: int(s))

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

        if args.dry_run:
            print(f"  [dry-run] paper {pid} ({video_id}): would update title → {meta['title']}")
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


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = p.add_subparsers(dest="cmd", required=True)

    sp_list = sub.add_parser("list", help="discover channel videos and save mapping.json")
    sp_list.set_defaults(func=cmd_list)

    sp_push = sub.add_parser("push", help="push metadata (+ thumbnails) to YouTube")
    sp_push.add_argument("--dry-run", action="store_true", help="preview without writing")
    sp_push.add_argument("--paper", help="push a single paper id, e.g. --paper 1")
    sp_push.add_argument(
        "--thumbnails", action="store_true", help="also upload thumbnail-{id}.png"
    )
    sp_push.set_defaults(func=cmd_push)

    args = p.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
