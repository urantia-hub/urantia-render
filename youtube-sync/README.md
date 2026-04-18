# youtube-sync

Bulk-update titles, descriptions, tags, and thumbnails on the 120 already-uploaded UrantiaHub videos using the YouTube Data API v3. Reads enriched metadata JSON from `../output/metadata/{N}.json`.

## Setup (one-time)

1. **Create a Google Cloud project.** https://console.cloud.google.com/ → "New Project" → name it `urantiahub-sync` or similar.
2. **Enable the YouTube Data API v3.** APIs & Services → Library → search "YouTube Data API v3" → Enable.
3. **Create OAuth consent screen.** APIs & Services → OAuth consent screen → External → fill in app name, support email, developer email. Add yourself as a test user.
4. **Create OAuth client credentials.** APIs & Services → Credentials → Create Credentials → OAuth client ID → Desktop app → name it `youtube-sync`.
5. **Download the client secret.** Click the download button on the credential row → save as `youtube-sync/client_secret.json`.

You'll need [uv](https://docs.astral.sh/uv/) installed (`brew install uv` or `curl -LsSf https://astral.sh/uv/install.sh | sh`).

## Usage

```bash
# One-time: map paper IDs to YouTube video IDs (writes mapping.json).
# First run opens a browser for OAuth consent.
./sync.py list

# Preview what would change without writing.
./sync.py push --dry-run

# Push titles + descriptions + tags to all mapped videos.
./sync.py push

# Push metadata AND upload the 1920x1080 thumbnails.
./sync.py push --thumbnails

# Test a single paper first.
./sync.py push --paper 1 --thumbnails
```

## Quota

YouTube Data API costs:
- `videos.update`: 50 units
- `thumbnails.set`: 50 units

Default daily quota is 10,000 units. All 120 videos × 100 units (metadata + thumbnail) = 12,000 units — one day over the limit, so either split across two days or request a quota bump from Google Cloud.

## Safety

- `mapping.json` is generated from your actual channel — inspect it before running `push`.
- `token.json` caches your OAuth refresh token. Do not commit it (it's in `.gitignore`).
- `--dry-run` prints the planned changes without calling the mutating API.
- YouTube keeps a title-change history, so edits are reversible through YouTube Studio even after push.
