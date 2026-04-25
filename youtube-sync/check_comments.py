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
Verify pinned-nav comments are present on every paper in mapping.json.

Lists the top comment thread on each video and checks for the marker
string `urantiahub.com/papers/` in the body. Quota cost: 1 unit per
video (200 papers ≈ 200 units).
"""
from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from sync import get_service, _expand_range  # noqa: E402
import json

MAPPING = HERE / "mapping.json"

MARKER = "urantiahub.com/papers/"


def check_video(yt, vid: str) -> tuple[bool, str]:
    """Returns (has_nav_comment, summary)."""
    try:
        resp = (
            yt.commentThreads()
            .list(part="snippet", videoId=vid, maxResults=20, order="relevance")
            .execute()
        )
    except Exception as e:
        return False, f"API error: {e!r}"

    for thread in resp.get("items", []):
        snip = thread["snippet"]["topLevelComment"]["snippet"]
        body = snip.get("textDisplay", "") + " " + snip.get("textOriginal", "")
        if MARKER in body:
            author = snip.get("authorDisplayName", "?")
            return True, f"by {author}"
    return False, f"no nav comment in top {len(resp.get('items', []))} threads"


def main() -> None:
    papers = sys.argv[1] if len(sys.argv) > 1 else "0-196"
    pids = _expand_range(papers)
    mp = {k: v for k, v in json.load(MAPPING.open()).items() if not k.startswith("_")}

    yt = get_service()
    missing = []
    skipped = []
    ok = 0

    for pid in pids:
        if pid not in mp:
            print(f"  paper {pid}: not on channel, skip")
            skipped.append(pid)
            continue
        has, info = check_video(yt, mp[pid])
        marker = "✓" if has else "✗"
        print(f"  paper {pid:>3} ({mp[pid]}): {marker} {info}")
        if has:
            ok += 1
        else:
            missing.append(pid)

    print(f"\nDone: {ok} have nav comments, {len(missing)} missing, {len(skipped)} not on channel")
    if missing:
        print(f"Missing on: {','.join(missing)}")


if __name__ == "__main__":
    main()
