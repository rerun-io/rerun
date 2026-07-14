"""Stream a Rerun catalog into PyTorch with the experimental dataloader."""

from __future__ import annotations

from pathlib import Path

import torch
import torch.multiprocessing
from torch import nn

import rerun as rr

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
dataset = client.create_dataset("my_robot_data", exist_ok=True)

uris = [f"file://{p.resolve()}" for p in rrd_paths]
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

source = DataSource(
    dataset=client.get_dataset("my_robot_data"),
    segments=[
        "ILIAD_50aee79f_2023_07_12_20h_55m_08s",
        "ILIAD_5e938e3b_2023_07_20_10h_40m_10s",
    ],
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
# Offsets are in the index timeline's native unit: integer steps for integer
# indices, or nanoseconds for timestamp indices (use multiples of the
# FixedRateSampling period).
windowed_action = Field(
    "/action/joint_positions:Scalars:scalars",
    decode=NumericDecoder(),
    window=(1, 50),
)
# endregion: window


# region: video_decoder
# Decode a compressed video stream as part of each sample.
# `keyframe_interval` must be at least the actual GOP length. For timestamp
# timelines, `fps_estimate` should also approximate the true frame rate.
from rerun.experimental.dataloader import VideoFrameDecoder

image_field = Field(
    "/camera/wrist:VideoStream:sample",
    decode=VideoFrameDecoder(
        codec="h264", keyframe_interval=500, fps_estimate=15.0
    ),
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
