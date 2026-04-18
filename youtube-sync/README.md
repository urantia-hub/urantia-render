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

## Auto-uploader daemon

`bin/urantia-uploader` runs `orchestrator.py` as a background process that
walks papers 0 → 196 in order, rendering each MP4 + thumbnail + metadata and
uploading to YouTube. It respects the daily API quota and auto-resumes after
a laptop reboot via one command.

### Control commands

```bash
./bin/urantia-uploader start      # launch in the background (nohup)
./bin/urantia-uploader stop       # kill the daemon (and any child render)
./bin/urantia-uploader restart
./bin/urantia-uploader pause      # PAUSED marker — daemon stays running, skips uploads
./bin/urantia-uploader resume
./bin/urantia-uploader status     # pid, progress, today's quota, last 10 log lines
./bin/urantia-uploader log        # tail -f orchestrator.log
./bin/urantia-uploader next       # show which paper is up next + queue
./bin/urantia-uploader mark-done <paper-id> <video-id>
                                  # record a manually-uploaded paper so the daemon skips it
```

### How render-ahead works

Rendering is local and doesn't touch the YouTube API quota. When the daily
budget (8,500 units, ~5 uploads) is spent, the orchestrator switches from
"upload" mode to "render-ahead" mode: it walks forward through remaining
papers and renders any that are missing MP4/metadata/thumbnail. By the time
the quota resets at midnight Pacific Time, the next day's uploads are already
sitting on disk and can go out back-to-back.

### Bypassing the API quota with manual uploads

Drag-and-drop uploads in YouTube Studio don't count against the API quota.
If the daemon has pre-rendered assets sitting in `output/videos/`,
`output/metadata/`, and `output/thumbnails/`, you can:

1. Upload the MP4 at `output/videos/tts-1-hd-nova-{pid}.mp4` via Studio.
2. Copy the title/description/tags from `output/metadata/{pid}.json`.
3. Attach the thumbnail at `output/thumbnails/thumbnail-{pid}.png`.
4. Add the video to the "All Papers" playlist and the relevant Part playlist
   (Foreword = All only; 1–31 = Part I; 32–56 = Part II; 57–119 = Part III;
   120–196 = Part IV).
5. Record the upload so the daemon skips that paper:

   ```bash
   ./bin/urantia-uploader mark-done 5 dQw4w9WgXcQ
   ```

This lets you upload faster than the 5/day API cap — useful during downtime
when you want to push a batch through by hand.

**Safety net:** even if you forget to run `mark-done`, the daemon refreshes
state from the channel before every upload cycle (costs ~5 quota units per
refresh). Any paper it finds on the channel that isn't already in
`upload-state.json` gets merged automatically, so it won't clobber a manual
upload with a duplicate.

### Reboot behavior

The daemon runs under `nohup`, not `launchd`, because macOS TCC restrictions
block launchd agents from accessing `~/Desktop` even with Full Disk Access
granted to `/bin/bash`. After a reboot, run `./bin/urantia-uploader start`
once to resume — all state (upload-state.json, quota-log.json, rendered
files) persists on disk.

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
