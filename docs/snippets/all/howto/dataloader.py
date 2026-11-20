"""Stream a Rerun catalog into PyTorch with the experimental dataloader."""

from __future__ import annotations

import warnings
from pathlib import Path

import torch
import torch.multiprocessing
from torch import nn

import rerun as rr

# Suppress warnings when FixedRateSampling samples indices where some entities
# lack data.
warnings.filterwarnings(
    "ignore",
    message=r"Skipping samples where field .* has no value",
    category=RuntimeWarning,
)

# Rerun's tokio runtime is not fork-safe, so DataLoader workers must use
# `spawn`. Set this before constructing any DataLoader, even with
# `num_workers=0`, so bumping the worker count later doesn't deadlock on the
# first catalog call.
torch.multiprocessing.set_start_method("spawn", force=True)

# In a real workflow you'd start a long-running OSS server (`rerun server`)
# and point a `CatalogClient` at it. For this self-contained snippet we use
# a short-lived in-process server and the DROID sample dataset shipped with
# the repo.
sample_5_path = (
    Path(__file__).parents[4] / "tests" / "assets" / "rrd" / "sample_5"
)
server = rr.server.Server()
rrd_paths = list(sample_5_path.glob("*.rrd"))

# region: register
client = rr.catalog.CatalogClient(server.url())
# On Rerun Hub, connect to your Hub instead:
# client = rr.catalog.CatalogClient("rerun://api.<stack>.cloud.rerun.io")
dataset = client.create_dataset("my_robot_data", exist_ok=True)

uris = [f"file://{p.resolve()}" for p in rrd_paths]
# On Rerun Hub, register from object storage:
# dataset.register(["s3://bucket/episode_0.rrd"]).wait()
dataset.register(uris).wait()
# endregion: register

# region: describe_sample
from rerun.experimental.dataloader import (
    DataSource,
    Field,
    FixedRateSampling,
    NumericDecoder,
    RerunIterableDataset,
)

segment_ids = [
    "ILIAD_50aee79f_2023_07_12_20h_55m_08s",
    "ILIAD_5e938e3b_2023_07_20_10h_40m_10s",
]
source = DataSource(
    dataset=client.get_dataset("my_robot_data"),
    segments=segment_ids,
)

fields = {
    "state": Field(
        "/observation/joint_positions:Scalars:scalars", decode=NumericDecoder()
    ),
    "action": Field(
        "/action/joint_positions:Scalars:scalars", decode=NumericDecoder()
    ),
}

ds = RerunIterableDataset(
    source=source,
    index="real_time",
    fields=fields,
    timeline_sampling=FixedRateSampling(rate_hz=15.0),
)
# endregion: describe_sample


# region: window
# Each sample now carries the next 50 action steps instead of a single value.
# The selected index above is a timestamp timeline, so these offsets are
# expressed in seconds.
# An integer index timeline would instead require integral index-unit offsets.
windowed_action = Field(
    "/action/joint_positions:Scalars:scalars",
    decode=NumericDecoder(),
    window=tuple(step / 15.0 for step in range(1, 51)),
)
# endregion: window


# region: video_decoder
# Decode a compressed video stream as part of each sample.
# The stream must include its sibling `VideoStream:is_keyframe` component.
from rerun.experimental.dataloader import VideoFrameDecoder

image_field = Field(
    "/camera/wrist:VideoStream:sample",
    decode=VideoFrameDecoder(codec="h264"),
)
# endregion: video_decoder


# region: dataloader
from torch.utils.data import DataLoader

from rerun.experimental.dataloader import RerunMapDataset


def my_collate(
    samples: list[dict[str, torch.Tensor]],
) -> dict[str, torch.Tensor]:
    # Drop samples that landed outside the underlying data (FixedRateSampling
    # may overshoot the end of a segment by one grid point).
    samples = [
        s for s in samples if s["state"].numel() > 0 and s["action"].numel() > 0
    ]
    return {
        "state": torch.stack([s["state"] for s in samples]).float(),
        "action": torch.stack([s["action"] for s in samples]).float(),
    }


loader = DataLoader(
    ds,
    batch_size=8,
    num_workers=0,
    shuffle=isinstance(ds, RerunMapDataset),  # iterable shuffles internally
    collate_fn=my_collate,
)
# endregion: dataloader


# A one-layer stand-in for the actual policy. The point of the snippet is
# the dataloader, not the model.
class TinyPolicy(nn.Module):
    def __init__(self, state_dim: int = 7, action_dim: int = 7) -> None:
        super().__init__()
        self.linear = nn.Linear(state_dim, action_dim)

    def forward(
        self, batch: dict[str, torch.Tensor]
    ) -> tuple[torch.Tensor, dict[str, float]]:
        prediction = self.linear(batch["state"])
        loss = nn.functional.mse_loss(prediction, batch["action"])
        return loss, {}


policy = TinyPolicy()
optimizer = torch.optim.AdamW(policy.parameters(), lr=1e-4)
device = torch.device("cpu")
policy.to(device)
epochs = 1

# region: train
for epoch in range(epochs):
    if isinstance(ds, RerunIterableDataset):
        ds.set_epoch(epoch)
    for batch in loader:
        batch = {k: v.to(device) for k, v in batch.items()}
        loss, _ = policy.forward(batch)
        loss.backward()
        optimizer.step()
        optimizer.zero_grad()
# endregion: train


# region: data_source
from rerun.experimental.spans import Curation

whole_source = DataSource(dataset)
view = dataset.filter_segments(segment_ids).filter_contents([
    "/observation/**",
    "/action/**",
])
view_source = DataSource(view)
curation = Curation.from_source(view, index="real_time")
curated_source = DataSource(dataset, curation=curation)
# endregion: data_source


# region: curate_recordings
from rerun.experimental.dataloader import (
    BlockShuffle,
    DataSource,
    FixedRateSampling,
    RerunIterableDataset,
)
from rerun.experimental.spans import Curation

dataset_entry = source.dataset
recording_view = dataset_entry.filter_segments(segment_ids)
curation = Curation.from_source(recording_view, index="real_time")
# endregion: curate_recordings

# region: curate_sql
ctx = dataset_entry.catalog.ctx
ctx.register_view("recordings", dataset_entry.segment_table())
selected_recordings = ctx.sql(
    "SELECT rerun_segment_id FROM recordings"
    ' WHERE "property:episode:success"[1] = true'
)
recording_view = dataset_entry.filter_segments(selected_recordings)
curation = Curation.from_source(recording_view, index="real_time")
# endregion: curate_sql

# region: save_curation
Curation.save(dataset_entry, curation.to_arrow(), tag="training-v1")
curation = Curation.open(dataset_entry, tag="training-v1")
source = DataSource(dataset_entry, curation=curation)

curated_ds = RerunIterableDataset(
    source=source,
    index="real_time",
    fields=fields,
    timeline_sampling=FixedRateSampling(rate_hz=30),
    shuffle_strategy=BlockShuffle(),
    seed=0,
)
# endregion: save_curation

first_sample = next(iter(curated_ds))
assert set(first_sample) == {"state", "action"}
