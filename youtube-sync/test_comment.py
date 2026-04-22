#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "google-api-python-client>=2.120",
#     "google-auth-oauthlib>=1.2",
#     "google-auth-httplib2>=0.2",
# ]
# ///
"""Smoke test for build_pinned_comment (minimalist shape).

Exercises edge cases (Foreword, final paper, cross-part boundaries, mid-
rollout with no next paper yet, only-Foreword uploaded). No YouTube API
calls. Run directly: `./test_comment.py`.
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from sync import build_pinned_comment  # noqa: E402

FAKE_PLAYLISTS = {
    "all": "PL_ALL",
    "part-1": "PL_P1",
    "part-2": "PL_P2",
    "part-3": "PL_P3",
    "part-4": "PL_P4",
}

FULL_DONE = {str(i): f"vid{i:03d}" for i in range(0, 197)}


def assert_in(needle: str, body: str, case: str) -> None:
    assert needle in body, f"[{case}] missing: {needle!r}\n--- body ---\n{body}"


def assert_not_in(needle: str, body: str, case: str) -> None:
    assert needle not in body, f"[{case}] unexpected: {needle!r}\n--- body ---\n{body}"


def line_order(body: str, first: str, second: str) -> bool:
    """Both substrings present and `first` appears before `second`."""
    i = body.find(first)
    j = body.find(second)
    return i != -1 and j != -1 and i < j


def main() -> None:
    # ── Full catalogue ──────────────────────────────────────────────────
    body = build_pinned_comment("62", FULL_DONE, FAKE_PLAYLISTS)
    assert_in("← Previous (Paper 61): https://youtu.be/vid061?list=PL_ALL", body, "62 full")
    assert_in("Next (Paper 63) →: https://youtu.be/vid063?list=PL_ALL", body, "62 full")
    assert_in("You're in Part III — The History of Urantia.", body, "62 full")
    assert_in("Read along: https://urantiahub.com/papers/62", body, "62 full")
    # Read along must live in the nav block, above the blank line, so it
    # stays above YouTube's "Read more" fold.
    assert line_order(body, "Read along:", "\n\n"), f"[62 full] Read along after blank:\n{body}"
    assert line_order(body, "\n\n", "You're in Part III"), f"[62 full] orientation not after blank:\n{body}"
    # Intentionally removed bits:
    assert_not_in("Navigate:", body, "62 full (no Navigate header)")
    assert_not_in("Full playlist:", body, "62 full (no full playlist line)")
    assert_not_in("Other parts", body, "62 full (no Other parts)")
    assert_not_in("playlist?list=", body, "62 full (no playlist URLs at all)")
    assert_not_in("final paper", body, "62 full (not final)")
    assert_not_in("{", body, "62 full (placeholders)")

    # ── Paper 1 ────────────────────────────────────────────────────────
    body = build_pinned_comment("1", FULL_DONE, FAKE_PLAYLISTS)
    assert_in("← Previous (Paper 0):", body, "1 full")
    assert_in("Next (Paper 2) →:", body, "1 full")
    assert_in("You're in Part I — The Central and Superuniverses.", body, "1 full")
    assert_in("Read along: https://urantiahub.com/papers/1", body, "1 full")

    # ── Part boundaries 31 / 32 ────────────────────────────────────────
    body = build_pinned_comment("31", FULL_DONE, FAKE_PLAYLISTS)
    assert_in("You're in Part I", body, "31 full")
    assert_in("Next (Paper 32) →:", body, "31 full")

    body = build_pinned_comment("32", FULL_DONE, FAKE_PLAYLISTS)
    assert_in("You're in Part II — The Local Universe.", body, "32 full")
    assert_in("← Previous (Paper 31):", body, "32 full")

    # ── Part boundaries 119 / 120 ──────────────────────────────────────
    body = build_pinned_comment("119", FULL_DONE, FAKE_PLAYLISTS)
    assert_in("You're in Part III", body, "119 full")
    body = build_pinned_comment("120", FULL_DONE, FAKE_PLAYLISTS)
    assert_in("You're in Part IV — The Life and Teachings of Jesus.", body, "120 full")

    # ── Foreword (0) ───────────────────────────────────────────────────
    body = build_pinned_comment("0", FULL_DONE, FAKE_PLAYLISTS)
    assert_not_in("← Previous", body, "0 full (no prev)")
    assert_in("Next (Paper 1) →:", body, "0 full")
    assert_in("You're reading the Foreword. The main text begins with Part I.", body, "0 full")
    assert_in("Read along: https://urantiahub.com/papers/0", body, "0 full")
    assert_not_in("Other parts", body, "0 full (no Other parts)")

    # ── Paper 196 (final) ──────────────────────────────────────────────
    body = build_pinned_comment("196", FULL_DONE, FAKE_PLAYLISTS)
    assert_in("← Previous (Paper 195):", body, "196 full")
    assert_not_in("Next (Paper", body, "196 full (no next)")
    assert_in("You're in Part IV — The Life and Teachings of Jesus.", body, "196 full")
    assert_in("Read along: https://urantiahub.com/papers/196", body, "196 full")
    assert_in("You've reached the final paper. Thanks for reading along!", body, "196 full")
    # Sign-off must come after the orientation.
    assert line_order(body, "You're in Part IV", "reached the final paper"), f"[196] wrong order"

    # ── Mid-rollout: only papers 0-100 uploaded ────────────────────────
    mid_done = {str(i): f"vid{i:03d}" for i in range(0, 101)}
    body = build_pinned_comment("100", mid_done, FAKE_PLAYLISTS)
    assert_in("← Previous (Paper 99):", body, "100 mid")
    assert_not_in("Next (Paper", body, "100 mid (N+1 not uploaded yet)")
    assert_in("You're in Part III", body, "100 mid")
    assert_in("Read along: https://urantiahub.com/papers/100", body, "100 mid")

    body = build_pinned_comment("0", mid_done, FAKE_PLAYLISTS)
    assert_in("Next (Paper 1) →:", body, "0 mid")

    # ── Degenerate: only Foreword uploaded ─────────────────────────────
    only_0 = {"0": "vid000"}
    body = build_pinned_comment("0", only_0, FAKE_PLAYLISTS)
    assert_not_in("← Previous", body, "0 lone")
    assert_not_in("Next (Paper", body, "0 lone (nothing uploaded after 0)")
    assert_in("You're reading the Foreword", body, "0 lone")
    assert_in("Read along: https://urantiahub.com/papers/0", body, "0 lone")
    # No stray blank line at the very top when both nav lines absent
    assert not body.startswith("\n"), f"[0 lone] leading blank line:\n{body!r}"

    # ── Placeholder scan across every paper id ─────────────────────────
    for i in range(0, 197):
        b = build_pinned_comment(str(i), FULL_DONE, FAKE_PLAYLISTS)
        assert "{" not in b and "}" not in b, f"placeholder leaked in paper {i}:\n{b}"

    print("all comment-builder checks passed")


if __name__ == "__main__":
    main()
