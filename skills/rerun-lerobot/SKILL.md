---
name: rerun-lerobot
description: Ingest a LeRobot (HuggingFace) dataset into Rerun. Read when converting a LeRobot dataset to RRDs, splitting it into per-episode segments, or registering it on a Rerun catalog. Covers the built-in directory importer (log_file_from_path), the RrdReader + send_chunks per-episode split, and when to drop to ParquetReader for custom control.
user_invocable: true
allowed-tools: Read, Grep, Bash, WebFetch
---

# Rerun LeRobot ingestion

Rerun has a **built-in LeRobot importer**: point `log_file_from_path` (or the viewer, or `rerun <dir>` on the CLI) at the dataset _directory_ and it ingests episodes, camera videos, and state/action tables with no conversion code.
There is no chunk-level `LeRobotReader`; the chunk-processing route is to import first, then reprocess the resulting RRD with `RrdReader`.

The download step needs
`huggingface_hub`.

## Step 1: dataset -> one combined RRD

```python
from huggingface_hub import snapshot_download
import rerun as rr

dataset_dir = snapshot_download(repo_id="rerun/so101-pick-and-place", repo_type="dataset", local_dir=dest)

with rr.RecordingStream("rerun_example_lerobot") as rec:
    rec.save(str(combined_rrd))
    rec.log_file_from_path(str(dataset_dir))  # the built-in importer
```

The importer emits one recording per episode (recording ids like `episode_1`), plus a metadata-only root recording, all into the single RRD.

`rr.RecordingStream` + `log_file_from_path` here is the **importer bootstrap** — the one place `RecordingStream` is correct in an ingestion pipeline (it drives the built-in importer, not per-message logging). Do not generalize it to `rr.log`-per-message loops; for everything after import, reprocess the RRD with `RrdReader` + lenses (see `rerun-chunk-processing`: Chunk API vs logging API).

## Step 2: split into per-episode RRDs

Catalog segments are one-recording-per-file, and `recording_id` becomes the segment id on registration.
Split with `RrdReader`:

```python
reader = rr.experimental.RrdReader(str(combined_rrd))
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

## Gotchas

1. `log_file_from_path` must target the dataset **root directory**, not a file inside it.
2. Unpadded episode ids sort incorrectly downstream; pad before registering.
3. The combined RRD contains a metadata-only root recording; skip stores with no entity paths or you register an empty segment.

## References

- `https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader` `prepare_dataset.py` (download → import → split → register, complete and runnable) and `train.py` (training-side consumption via `rerun.experimental.dataloader`)
