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

## Clean-slate re-upload workflow (after a 4K batch render)

Once the AWS batch has finished and you've synced MP4s to `../output/videos/`:

```bash
# 0. Make sure mapping.json is current (reflects the 120 old uploads).
./sync.py list

# 1. Create / find the 5 playlists (idempotent; writes playlists.json).
./sync.py playlists

# 2. Delete the old 1080p uploads. Preview first, then commit with --yes.
./sync.py delete
./sync.py delete --yes

# 3. Upload the new 4K set. Each upload auto-assigns to "All Papers" + the
#    relevant Part playlist based on paper ID.
./sync.py upload --papers 0-196 --dry-run
./sync.py upload --papers 0-196
```

Re-running `upload` is safe — already-uploaded papers (tracked in
`upload-state.json`) get skipped unless you pass `--force`. If the daily
quota trips mid-upload, just re-run the same command tomorrow.

## Quota

YouTube Data API costs (per paper):
- `videos.update`: 50 units
- `thumbnails.set`: 50 units
- `videos.insert`: 1,600 units
- `videos.delete`: 50 units
- `playlistItems.insert`: 50 units × 2 playlists = 100 units

**Full clean-slate operation** (delete 120 + upload 197 with thumbnails + 2 playlists each):
- deletes: 120 × 50 = 6,000
- uploads: 197 × 1,750 = ~344,750 units

At the default 10,000/day quota, that's **~35 days** at full throttle. Request a quota increase at https://console.cloud.google.com/apis/api/youtube.googleapis.com/quotas — a ~500k/day limit is typical for approved creator-tools. Alternatively, spread uploads across weeks at 5/day.

## Safety

- `mapping.json` is generated from your actual channel — inspect it before running `push`.
- `token.json` caches your OAuth refresh token. Do not commit it (it's in `.gitignore`).
- `--dry-run` prints the planned changes without calling the mutating API.
- YouTube keeps a title-change history, so edits are reversible through YouTube Studio even after push.
