#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "openai>=1.40",
#     "python-dotenv>=1.0",
# ]
# ///
"""
Generate the 6 trailer narration clips with OpenAI TTS (nova / tts-1-hd).

Outputs:
  output/trailer/audio/cold-open.mp3
  output/trailer/audio/part-1.mp3
  output/trailer/audio/part-2.mp3
  output/trailer/audio/part-3.mp3
  output/trailer/audio/part-4.mp3
  output/trailer/audio/cta.mp3

Reads OPENAI_API_KEY from urantia-render/.env. Each clip is generated in a
single API call so timing/cadence is consistent across the set. Cost is
roughly $0.02 total at tts-1-hd pricing ($30/M chars × ~700 chars).
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

from dotenv import load_dotenv
from openai import OpenAI

HERE = Path(__file__).resolve().parent
REPO = HERE.parent
OUT_DIR = REPO / "output" / "trailer" / "audio"

load_dotenv(REPO / ".env")
api_key = os.environ.get("OPENAI_API_KEY")
if not api_key:
    sys.exit("OPENAI_API_KEY not set (checked urantia-render/.env)")

CLIPS: dict[str, str] = {
    "cold-open": "The Urantia Book. All 197 Papers. Read along.",
    "part-1": "Part One. The Central and Superuniverses.",
    "part-2": "Part Two. The Local Universe.",
    "part-3": "Part Three. The History of Urantia.",
    "part-4": "Part Four. The Life and Teachings of Jesus.",
    "cta": "Read the Urantia Book. Subscribe.",
}

MODEL = "tts-1-hd"
VOICE = "nova"


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    client = OpenAI(api_key=api_key)

    for slug, text in CLIPS.items():
        out_path = OUT_DIR / f"{slug}.mp3"
        print(f"  {slug}: '{text}'")
        with client.audio.speech.with_streaming_response.create(
            model=MODEL,
            voice=VOICE,
            input=text,
            response_format="mp3",
        ) as resp:
            resp.stream_to_file(out_path)
        size = out_path.stat().st_size
        print(f"    → {out_path.relative_to(REPO)} ({size:,} bytes)")

    print(f"\nDone. Listen with: open {OUT_DIR}")


if __name__ == "__main__":
    main()
