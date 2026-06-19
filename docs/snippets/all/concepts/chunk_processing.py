# region: setup
from __future__ import annotations

import math
import uuid
from collections.abc import Callable
from pathlib import Path

import pyarrow as pa
import pyarrow.compute as pc

import rerun as rr
from rerun.experimental import (
    Chunk,
    DeriveLens,
    LazyChunkStream,
    McapReader,
    Selector,
)

MCAP = (
    Path(__file__).resolve().parents[4]
    / "tests"
    / "assets"
    / "mcap"
    / "trossen_transfer_cube.mcap"
)
OUT = Path("chunk_processing.rrd")
# endregion: setup


# region: reading
stream = McapReader(MCAP).stream()
# endregion: reading


# region: processing
JOINTS = [
    "waist",
    "shoulder",
    "elbow",
    "forearm_roll",
    "wrist_angle",
    "wrist_rotate",
]


def pick_joint(i: int) -> Callable[[pa.Array], pa.Array]:
    """Extract joint `i` from a list<float64> column and convert rad → deg."""
    return lambda arr: pc.multiply(pc.list_element(arr, i), 180.0 / math.pi)


def fan(side: str) -> list[DeriveLens]:
    return [
        DeriveLens(
            "schemas.proto.JointState:message",
            output_entity=f"/joints_deg/{side}/{name}",
        ).to_component(
            rr.Scalars.descriptor_scalars(),
            Selector(".joint_positions").pipe(pick_joint(i)),
        )
        for i, name in enumerate(JOINTS)
    ]


processed = (
    stream
    .drop(content="/video_raw/**")
    .lenses(
        fan("left"),
        content="/robot_left/**",
        output_mode="forward_unmatched",
    )
    .lenses(
        fan("right"),
        content="/robot_right/**",
        output_mode="forward_unmatched",
    )
)
# endregion: processing

# TODO(ab): change this to merge properties instead, when we have proper
# interop between logging SDK and py-chunk

# region: merging
metadata = Chunk.from_columns(
    "/metadata",
    indexes=[],
    columns=rr.AnyValues.columns(
        processing_type="ingestion",
        processing_version="v1",
    ),
)
merged = LazyChunkStream.merge(processed, LazyChunkStream.from_iter([metadata]))
# endregion: merging


# region: write
merged.write_rrd(
    OUT,
    application_id="rerun_example_chunk_processing",
    recording_id=str(uuid.uuid4()),
)
# endregion: write
