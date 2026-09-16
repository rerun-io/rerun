---
name: rerun-lerobot
description: Ingest a LeRobot (HuggingFace) dataset into Rerun. Read when converting a LeRobot dataset to RRDs, splitting it into per-episode segments, or registering it on a Rerun catalog. Covers the built-in directory importer (log_file_from_path), the per-episode LeRobotReader, and when to drop to ParquetReader for custom control.
user_invocable: true
allowed-tools: Read, Grep, Bash, WebFetch
---

# Rerun LeRobot ingestion

There are two ways in, and which one you want depends on whether you are viewing or converting.

- **Just viewing?** Rerun has a **built-in LeRobot importer**: point `log_file_from_path` (or the viewer, or `rerun <dir>` on the CLI) at the dataset _directory_ and it ingests episodes, camera videos, and state/action tables with no conversion code.
- **Converting?** `rr.experimental.LeRobotReader` reads the dataset one episode at a time as a lazy chunk stream, which is what you want for per-episode RRDs and for lenses.

The download step needs `huggingface_hub`.

## Step 0: pick a dataset and check the HF token

Both scripts below live in `scripts/` next to this file, and run standalone through `uv` — they declare `huggingface_hub` inline, so there is no environment to set up.

Two facts decide what a dataset costs to fetch, and the Hub search results show neither.
Get both, for every candidate, in one call:

```bash
uv run scripts/find_lerobot_dataset.py <query> --robot-type <robot> --want 5
```

**`codebase_version` decides whether a partial download is possible at all.**

- **v2.1 and earlier** store one parquet and one mp4 per episode (`video_path` ends in `episode_{episode_index:06d}.mp4`), so "just the first N episodes" downloads only those episodes.
- **v3.0** concatenates every episode into a few large files (`video_path` ends in `file-{file_index:03d}.mp4`), so any subset costs the whole download.
  If the user asked for a few episodes, prefer a v2.1 dataset — for the _download_ only.
  `LeRobotReader` reads a single episode out of either version once the files are local.

**How camera frames are stored decides the size**, and it varies by two orders of magnitude.
The `frames` and `MB/ep` columns report it:

- `mp4`: a `video`-dtype feature, one file per episode beside the parquet.
- `inline`: an `image`-dtype feature, raw frames inside the episode parquet, which is far larger for the same footage.

Both scripts report the actual size for the dataset in hand, so use that number rather than guessing.
Tell the user the size before you start the download, and prefer `mp4` unless they asked for something only the other set has.

`HF_TOKEN` is not required for public datasets, but without it the Hub rate-limits and throttles downloads.
If it is unset, say so and point the user at <https://huggingface.co/settings/tokens> to create a read token, then `export HF_TOKEN=hf_…` (or `hf auth login`).
Do not block on it — carry on unauthenticated and let the user decide.

```bash
[ -n "$HF_TOKEN" ] || echo "HF_TOKEN unset — downloads will be throttled"
```

## Step 1: dataset -> one combined RRD

Only needed if you want every episode in one file — to hand someone a single RRD, or to reprocess data you cannot re-read from the dataset directory.
For per-episode segments, skip to Step 2 and let `LeRobotReader` read the dataset directly.

```python
from huggingface_hub import snapshot_download
import rerun as rr

dataset_dir = snapshot_download(repo_id=repo_id, repo_type="dataset", local_dir=dest)

with rr.RecordingStream("rerun_example_lerobot") as rec:
    rec.save(str(combined_rrd))
    rec.log_file_from_path(str(dataset_dir))  # the built-in importer
```

The importer emits one recording per episode (recording ids like `episode_1`), plus a metadata-only root recording, all into the single RRD.

`rr.RecordingStream` + `log_file_from_path` here is the **importer bootstrap** — the one place `RecordingStream` is correct in an ingestion pipeline (it drives the built-in importer, not per-message logging). Do not generalize it to `rr.log`-per-message loops; for everything after import, reprocess with a reader + lenses (see `rerun-chunk-processing`: Chunk API vs logging API).

### Downloading only the first few episodes

```bash
uv run scripts/fetch_lerobot_episodes.py <repo_id> --episodes 5   # prints the dataset directory
```

It reports the download size before starting, downloads `meta/*` plus the wanted `episode_%06d` files, trims `meta/` to match, verifies that exactly the requested episodes landed, and prints what it got.
Its last line is the dataset directory; do not follow it with `find` or `du`.
Do the equivalent by hand and three things bite:

- The importer iterates the episode list in `meta/`, so an untrimmed `meta/` warns once per absent episode.
- `snapshot_download` writes into an existing directory, so episodes left by an earlier run with a different N survive and inflate the result.
- Both failures are invisible until you count the recordings in the viewer, which is why the script asserts instead of leaving you to look.

### Just looking at it in the viewer?

Skip the RRD entirely — the viewer imports the dataset directory itself.
Pass the plain path (`rerun /path/to/dataset`, or `open_url` over MCP).
Each episode arrives as its own recording, with `recording_id` `episode_0`, `episode_1`, ….

## Step 2: one RRD per episode

Catalog segments are one-recording-per-file, and `recording_id` becomes the segment id on registration.

`LeRobotReader` goes straight there, without building the combined RRD of Step 1 first:

```python
reader = rr.experimental.LeRobotReader(dataset_dir)  # v2 or v3
for episode in reader.episodes():
    episode_id = f"episode_{episode:05d}"  # zero-padded; see below
    reader.stream(episode).write_rrd(
        rrd_dir / f"{episode_id}.rrd",
        application_id="my_dataset",
        recording_id=episode_id,
    )
```

Streaming is lazy end to end, so memory is bounded by chunk size rather than by episode or dataset size, and video is cut to the episode's time window.
B-frame-free video (AV1, LeRobot's default codec) streams directly; a stream that must be re-encoded — H.264 with B-frames, or a window starting mid-GOP — needs `ffmpeg` on the `PATH`.
`stream()` also takes `entity_path_prefix`, `timeline`, and `video_mode="skip"` to drop video entirely.

To add lenses, drop topics, or fix data, put them between the stream and the write — see `rerun-chunk-processing`.

### Splitting an RRD you already have

If the combined RRD of Step 1 already exists — or the data came from somewhere other than the dataset directory — split it with `RrdReader` instead:

```python
reader = rr.chunk.RrdReader(str(combined_rrd))
for entry in reader.recordings():
    store = reader.store(store=entry)
    if not store.schema().entity_paths():  # skip the metadata-only root recording
        continue
    episode_id = zero_pad(entry.recording_id)  # episode_1 -> episode_00001
    with rr.RecordingStream("rerun_example_lerobot", recording_id=episode_id, send_properties=False) as rec:
        rec.save(str(rrd_dir / f"{episode_id}.rrd"))
        rec.send_chunks(store)
```

Two non-obvious moves:

- **Zero-pad the episode id.** `episode_10` sorts before `episode_2`
  lexicographically; segment tables and viewers sort lexicographically. Pad to
  a fixed width when re-assigning `recording_id`.
- **`send_properties=False`** on the new stream, so the copy doesn't inject
  fresh recording properties on top of the copied chunks.

`send_chunks` does not preserve the source store's identity; the new stream's
`recording_id` wins, which is exactly what makes the rename work.

If episodes need cleanup (drop topics, fix data, add derived components), run the store through lenses between read and write: `reader.stream(store=entry).drop(...).lenses(...)` then `collect().write_rrd(..., recording_id=episode_id)` (see `rerun-chunk-processing`).

Computed layers and per-episode properties then follow the standard patterns in `rerun-data-model` (layer `recording_id` must equal the episode segment id).

## Step 3: add the robot model (URDF)

A LeRobot dataset ships joint values, never a robot.
Without a URDF you get camera views and joint plots but no 3D arm, which is usually not what the user pictured when they asked to look at the data.
Adding one is a forward-kinematics layer — `rerun-urdf` owns that; this section is only the LeRobot-specific wiring.

- **The URDF.** `meta/info.json` `robot_type` names it (`so100`, `so101`, `aloha`, …); `rerun-urdf` has the source table.
- **The joint names.** `meta/info.json` `features.observation.state.names` lists them in column order.
  Map each source name explicitly to the corresponding URDF joint name; do not assume that removing a prefix produces a valid URDF name.
- **The values.** `observation.state` is the measured pose and the one to run FK on; `action` is the commanded pose and belongs on the plots, not the robot.
- **The units and calibration.** Confirm the units, sign, and offset against the dataset and robot documentation.
  Run `rerun-urdf`'s `check_joint_mapping.py` against the episode parquets before trusting a pose.

The FK source is the per-episode parquet, read directly rather than out of the imported RRD:

```python
ParquetReader(str(episode_parquet)).stream(
    column_grouping="individual",
    index_columns=[IndexColumn.sequence("frame_index")],
).drop(content="/__properties/**")
```

`frame_index` is the index the importer uses too, so the FK transforms line up with the video and plots.
Merge the URDF model stream plus the FK stream into each episode's RRD, keeping the episode's `recording_id` (see `rerun-urdf`'s "Minimal shape").
For a whole dataset, the model belongs in a shared asset rather than in every segment — also `rerun-urdf`.

## Gotchas

1. `log_file_from_path` must target the dataset **root directory**, not a file inside it. So does `LeRobotReader`.
2. Unpadded episode ids sort incorrectly downstream; pad before registering.
3. The combined RRD contains a metadata-only root recording; skip stores with no entity paths or you register an empty segment.
4. A partially-downloaded dataset imports fine, but warns once per episode listed in `meta/` and missing on disk.
5. Downloading a subset into a directory an earlier run already populated leaves the older episodes in place; start from an empty directory.
6. `image`-dtype (inline) datasets can be ~100x larger per episode than `mp4` ones; check before downloading, not after.
7. Check signs and units - e.g. an `observation.state` in degrees could produce wrong FK when fed into a robot model that expects radians.

## References

- `rerun-chunk-processing` (`LazyChunkStream`, lenses, `RrdReader`)
- `rerun-urdf` (adding the robot model and the joint-value mapping)
- `https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader` `prepare_dataset.py` (download → import → split → register, complete and runnable) and `train.py` (training-side consumption via `rerun.experimental.dataloader`)
