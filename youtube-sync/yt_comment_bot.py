#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "playwright>=1.40",
#     # sync.py is imported at module level and pulls in google libs:
#     "google-api-python-client>=2.120",
#     "google-auth-oauthlib>=1.2",
#     "google-auth-httplib2>=0.2",
# ]
# ///
"""
Playwright bot to post + pin nav comments on YouTube videos.

Bypasses YouTube's commentThreads.insert restriction on drag-and-drop
re-uploaded videos by driving the actual web UI instead of the Data API.

Recommended mode: connect to an already-running Chrome (avoids Google's
"browser or app may not be secure" anti-automation block on Playwright's
bundled Chromium).

Setup:
  1. Quit Chrome completely (cmd+q).
  2. Relaunch from terminal with the remote debugging port enabled:
       open -na "Google Chrome" --args --remote-debugging-port=9222
  3. In that Chrome, log into YouTube as usual (you're probably already in).
  4. Run the bot:
       ./yt_comment_bot.py --papers 17 --cdp
       ./yt_comment_bot.py --papers 18-61 --cdp

Dry-run any time (no browser needed):
  ./yt_comment_bot.py --papers 17 --dry-run
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from playwright.sync_api import sync_playwright, TimeoutError as PWTimeout

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from sync import build_pinned_comment, _expand_range  # noqa: E402

AUTH_STATE = HERE / ".yt-auth-state.json"
MAPPING = HERE / "mapping.json"
PLAYLISTS = HERE / "playlists.json"


def login_flow(p) -> None:
    """Open headed browser, wait for manual login, save auth state."""
    browser = p.chromium.launch(headless=False)
    ctx = browser.new_context()
    page = ctx.new_page()
    page.goto("https://www.youtube.com/")
    print("\nLog into YouTube in the opened browser. When you can see your")
    print("avatar in the top-right, press ENTER here to save the session.\n")
    input()
    ctx.storage_state(path=str(AUTH_STATE))
    print(f"Saved auth state to {AUTH_STATE}")
    browser.close()


def post_and_pin(page, video_id: str, comment_body: str) -> None:
    """Navigate to a video, post the comment, pin it. Raises on failure."""
    print(f"    → loading https://www.youtube.com/watch?v={video_id}")
    page.goto(
        f"https://www.youtube.com/watch?v={video_id}",
        wait_until="domcontentloaded",
    )

    # Wait for the comments header section to appear in the DOM. YouTube
    # lazy-loads it once you scroll, so trigger a scroll first.
    print(f"    → scrolling to comments")
    for _ in range(4):
        page.mouse.wheel(0, 1500)
        page.wait_for_timeout(700)
    page.wait_for_selector("ytd-comments-header-renderer", timeout=15_000)
    page.locator("ytd-comments-header-renderer").first.scroll_into_view_if_needed()
    page.wait_for_timeout(1000)

    # Click the simplebox placeholder to open the editor.
    print(f"    → opening comment editor")
    placeholder = page.locator(
        "ytd-comment-simplebox-renderer #placeholder-area"
    ).first
    placeholder.click(timeout=15_000)
    page.wait_for_timeout(500)

    # Type the comment.
    print(f"    → typing comment ({len(comment_body)} chars)")
    editor = page.locator("#contenteditable-root").first
    editor.click()
    editor.type(comment_body, delay=8)
    page.wait_for_timeout(500)

    # Click the Comment submit button.
    print(f"    → submitting")
    submit = page.locator(
        'button[aria-label="Comment"]:not([aria-disabled="true"])'
    ).first
    submit.click(timeout=10_000)
    # Scope to the visible main comments section. YouTube also renders an
    # off-screen "engagement-panel" copy of the comment thread that is never
    # visible, and Playwright's .first would pick that one.
    page.wait_for_selector(
        "ytd-comment-thread-renderer:not([engagement-panel])",
        timeout=15_000,
        state="visible",
    )
    page.wait_for_timeout(1500)

    # Open the action menu on the first comment. The three-dot button only
    # becomes visible on hover, so hover the comment row first.
    print(f"    → hovering first comment to reveal action menu")
    first_comment = page.locator(
        "ytd-comment-thread-renderer:not([engagement-panel])"
    ).first
    first_comment.scroll_into_view_if_needed()
    first_comment.hover()
    page.wait_for_timeout(700)
    print(f"    → opening action menu")
    page.locator(
        'ytd-comment-thread-renderer:not([engagement-panel]) '
        'button[aria-label="Action menu"]'
    ).first.click(force=True, timeout=10_000)
    page.wait_for_timeout(500)

    # Click "Pin" in the dropdown.
    print(f"    → clicking Pin")
    page.locator('tp-yt-paper-item:has-text("Pin")').first.click()
    page.wait_for_timeout(500)

    # Confirmation dialog (sometimes shown).
    try:
        page.locator('button:has-text("Pin")').last.click(timeout=3_000)
        print(f"    → confirmed pin dialog")
    except PWTimeout:
        pass
    page.wait_for_timeout(1000)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--login", action="store_true", help="Run the login flow")
    ap.add_argument("--papers", help="e.g. 17-61, 17,20,30")
    ap.add_argument(
        "--cdp",
        action="store_true",
        help=(
            "Connect to an already-running Chrome at localhost:9222 instead of "
            "launching a fresh Playwright Chromium. Avoids Google's anti-automation "
            "block. Requires Chrome to be started with --remote-debugging-port=9222."
        ),
    )
    ap.add_argument(
        "--cdp-port",
        type=int,
        default=9222,
        help="CDP port (default: 9222)",
    )
    ap.add_argument(
        "--headless",
        action="store_true",
        help="Run headless when launching a fresh browser (ignored with --cdp)",
    )
    ap.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the comment body for each paper, no browser/network calls",
    )
    args = ap.parse_args()

    with sync_playwright() as p:
        if args.login:
            login_flow(p)
            return

        if not args.papers:
            sys.exit("--papers is required (e.g. --papers 17-61)")

        mp = {
            k: v
            for k, v in json.load(MAPPING.open()).items()
            if not k.startswith("_")
        }
        playlists = json.load(PLAYLISTS.open())
        paper_ids = _expand_range(args.papers)

        if args.dry_run:
            for pid in paper_ids:
                if pid not in mp:
                    print(f"=== paper {pid}: not on channel, skip ===")
                    continue
                body = build_pinned_comment(pid, mp, playlists)
                print(f"\n=== paper {pid} ({mp[pid]}) ===\n{body}")
            return

        if args.cdp:
            try:
                browser = p.chromium.connect_over_cdp(f"http://localhost:{args.cdp_port}")
            except Exception as e:
                sys.exit(
                    f"Couldn't connect to Chrome on port {args.cdp_port}: {e}\n"
                    f"Quit Chrome and relaunch with:\n"
                    f"  open -na 'Google Chrome' --args --remote-debugging-port={args.cdp_port}"
                )
            # Reuse the existing logged-in context
            ctx = browser.contexts[0] if browser.contexts else browser.new_context()
            page = ctx.pages[0] if ctx.pages else ctx.new_page()
        else:
            if not AUTH_STATE.exists():
                sys.exit("No auth state — run with --login first, or use --cdp.")
            browser = p.chromium.launch(headless=args.headless)
            ctx = browser.new_context(storage_state=str(AUTH_STATE))
            page = ctx.new_page()

        ok = fail = skip = 0
        for pid in paper_ids:
            if pid not in mp:
                print(f"  paper {pid}: not on channel, skipping")
                skip += 1
                continue
            body = build_pinned_comment(pid, mp, playlists)
            print(f"  paper {pid}: {mp[pid]}")
            try:
                post_and_pin(page, mp[pid], body)
                print(f"    ✓ posted + pinned")
                ok += 1
            except Exception as e:
                print(f"    ✗ FAILED: {type(e).__name__}: {e}")
                fail += 1

        if not args.cdp:
            # Only close the browser if we launched it ourselves; in --cdp
            # mode the browser belongs to the user.
            browser.close()
        print(f"\nDone: {ok} posted, {fail} failed, {skip} skipped")


if __name__ == "__main__":
    main()
