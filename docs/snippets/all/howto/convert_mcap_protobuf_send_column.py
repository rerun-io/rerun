"""
Convert custom MCAP Protobuf messages to Rerun format using send_columns.

This example shows how to read MCAP files containing custom Protobuf messages
 and convert them to Rerun archetypes. It demonstrates:
- Converting transform messages
- Converting camera calibration to Pinhole
- Converting compressed video streams
- Using DynamicArchetype as a fallback for arbitrary protobuf messages
"""

from __future__ import annotations

import argparse

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from google.protobuf.json_format import MessageToDict
from mcap.reader import make_reader
from mcap_protobuf.decoder import DecoderFactory
import numpy as np
import rerun as rr


# --- Data types ---


@dataclass
class McapMessage:
    """Wrapper for decoded MCAP message data."""

    topic: str
    log_time_ns: int
    publish_time_ns: int
    proto_msg: Any


@dataclass
class VideoFrameData:
    """Parsed video frame data ready for collection."""

    log_time_ns: int
    publish_time_ns: int
    data: bytes
    codec: Any


@dataclass
class TransformData:
    """Parsed transform data ready for collection."""

    entity_path: str
    log_time_ns: int
    publish_time_ns: int
    translation: tuple[float, float, float]
    quaternion: tuple[float, float, float, float]
    parent_frame: str
    child_frame: str


class ColumnCollector:
    """Collects time-series data for sending via send_columns API.

    Handles both regular archetypes (VideoStream, Transform3D, etc.) and DynamicArchetype.
    - For regular archetypes: ColumnCollector(path, rr.VideoStream)
    - For DynamicArchetype: ColumnCollector(path, rr.DynamicArchetype, archetype_name="custom.CompressedImage")
    """

    def __init__(self, entity_path: str, archetype_type: type, archetype_name: str | None = None):
        self.entity_path = entity_path
        self.archetype_type: type = archetype_type
        self.archetype_name = archetype_name  # Only used for DynamicArchetype
        self.indexes: dict[str, list[int]] = {}
        self.components: dict[str, list[Any]] = {}

    def append(self, indexes: dict[str, int], **components: Any) -> None:
        """Append a row of data with time indexes and component values."""
        for name, value in indexes.items():
            self.indexes.setdefault(name, []).append(value)
        for name, value in components.items():
            self.components.setdefault(name, []).append(value)

    def send(self, rec: rr.RecordingStream, **kwargs: Any) -> None:
        """Send collected data via send_columns API."""
        if not self.indexes:
            return

        time_columns = [
            rr.TimeColumn(name, timestamp=[np.datetime64(t, "ns") for t in timestamps])
            for name, timestamps in self.indexes.items()
        ]

        if self.archetype_type is rr.DynamicArchetype:
            columns = rr.DynamicArchetype.columns(archetype=self.archetype_name, components=self.components)  # type: ignore[arg-type]
            kwargs.setdefault("strict", True)
        else:
            # .columns() is code-generated per-archetype with type-specific signatures, not on base class.
            #  So mypy can't verify it exists on 'type'.
            columns = self.archetype_type.columns(**self.components)  # type: ignore[attr-defined]

        rec.send_columns(
            self.entity_path,
            indexes=time_columns,
            columns=columns,
            **kwargs,
        )


# --- Message handlers ---


def convert_timestamp(secs: int, nanos: int) -> np.datetime64:
    """Convert ROS2 timestamp to nanoseconds since epoch."""
    epoch_nanos = secs * 1_000_000_000 + nanos
    return np.datetime64(epoch_nanos, "ns")


def camera_calibration(rec: rr.RecordingStream, msg: McapMessage, logged: set[str]) -> bool:
    """Convert CameraCalibration messages to Rerun Pinhole. Logs statically."""
    if msg.proto_msg.DESCRIPTOR.name != "CameraCalibration":
        return False

    if msg.topic in logged:
        return True
    logged.add(msg.topic)

    info = msg.proto_msg
    # Use from_fields to set parent_frame directly on the Pinhole
    # This connects the pinhole to the named transform frame without needing a separate Transform3D
    camera_info = rr.Pinhole.from_fields(
        image_from_camera=info.K,
        resolution=(info.width, info.height),
        parent_frame=info.frame_id,
    )
    rec.log(msg.topic, camera_info, static=True)
    return True


def compressed_video(msg: McapMessage) -> VideoFrameData | None:
    """Extract video frame data from CompressedVideo messages."""
    if msg.proto_msg.DESCRIPTOR.name != "CompressedVideo":
        return None

    return VideoFrameData(
        log_time_ns=msg.log_time_ns,
        publish_time_ns=msg.publish_time_ns,
        data=msg.proto_msg.data,
        codec=msg.proto_msg.format,
    )


def transform_msg(rec: rr.RecordingStream, msg: McapMessage, logged: set[tuple[str, str]]) -> list[TransformData]:
    """Extract transform data from FrameTransforms messages. Static transforms are logged immediately."""
    if msg.proto_msg.DESCRIPTOR.name != "FrameTransforms":
        return []

    static_topics = {"transforms_static"}
    is_static = msg.topic in static_topics
    result: list[TransformData] = []

    for transform in msg.proto_msg.transforms:
        parent = transform.parent_frame_id
        child = transform.child_frame_id
        translation = (transform.translation.x, transform.translation.y, transform.translation.z)
        quaternion = (transform.rotation.x, transform.rotation.y, transform.rotation.z, transform.rotation.w)
        entity_path = f"transforms/{child}"

        if is_static:
            key = (parent, child)
            if key in logged:
                continue
            logged.add(key)
            rec.log(
                entity_path,
                rr.Transform3D(
                    translation=translation,
                    quaternion=rr.Quaternion(xyzw=quaternion),
                    parent_frame=parent,
                    child_frame=child,
                ),
                static=True,
            )
        else:
            result.append(
                TransformData(
                    entity_path=entity_path,
                    log_time_ns=msg.log_time_ns,
                    publish_time_ns=msg.publish_time_ns,
                    translation=translation,
                    quaternion=quaternion,
                    parent_frame=parent,
                    child_frame=child,
                )
            )

    return result


def implicit_collect(msg: McapMessage) -> tuple[str, str, dict[str, Any]]:
    """Fallback: extract any protobuf message as DynamicArchetype data."""
    contents = MessageToDict(
        msg.proto_msg,
        preserving_proto_field_name=True,
        always_print_fields_with_no_presence=True,
    )
    return msg.proto_msg.DESCRIPTOR.full_name, msg.topic, contents


def send_collected_columns(rec: rr.RecordingStream, *collector_maps: dict[str, ColumnCollector]) -> None:
    """Send all collected time-series data using columnar API."""
    for collectors in collector_maps:
        for collector in collectors.values():
            collector.send(rec)


# --- Main execution ---

parser = argparse.ArgumentParser(description="Convert MCAP Protobuf messages to Rerun format.")
parser.add_argument("mcap_file", help="Path to the MCAP file to convert")
parser.add_argument(
    "--urdf-dir",
    type=Path,
    help="Directory containing robot URDF files (optional)",
)
args = parser.parse_args()

path_to_mcap = args.mcap_file

with rr.RecordingStream("rerun_example_convert_mcap_protobuf_send_column") as rec:
    rec.save("convert_mcap_protobuf_send_column.rrd")

    # Connect the viewer's root to the "world" frame.
    rec.log("/", rr.CoordinateFrame("world"), static=True)

    # Load all URDF files from directory if provided
    if args.urdf_dir:
        for urdf_path in args.urdf_dir.glob("*.urdf"):
            rec.log_file_from_path(urdf_path, static=True)
        rec.flush()  # Ensure URDFs finish loading before processing messages

    # State for deduplicating static logs
    logged_static_transforms: set[tuple[str, str]] = set()
    logged_static_calibrations: set[str] = set()

    # Collectors for time-series data
    video_collectors: dict[str, ColumnCollector] = {}
    transform_collectors: dict[str, ColumnCollector] = {}
    dynamic_collectors: dict[str, ColumnCollector] = {}

    with open(path_to_mcap, "rb") as f:
        reader = make_reader(f, decoder_factories=[DecoderFactory()])
        for _schema, channel, message, proto_msg in reader.iter_decoded_messages():
            msg = McapMessage(
                topic=channel.topic,
                log_time_ns=message.log_time,
                publish_time_ns=message.publish_time,
                proto_msg=proto_msg,
            )

            # Static-only: camera calibration
            if camera_calibration(rec, msg, logged_static_calibrations):
                continue

            # Time-series: compressed video
            if frame := compressed_video(msg):
                entity_path = msg.topic
                if entity_path not in video_collectors:
                    video_collectors[entity_path] = ColumnCollector(entity_path, rr.VideoStream)
                    rec.log(entity_path, rr.VideoStream(codec=frame.codec), static=True)
                video_collectors[entity_path].append(
                    indexes={"message_log_time": frame.log_time_ns, "message_publish_time": frame.publish_time_ns},
                    sample=frame.data,
                )
                continue

            # Time-series: transforms (static transforms logged inside handler)
            if transforms := transform_msg(rec, msg, logged_static_transforms):
                for t in transforms:
                    if t.entity_path not in transform_collectors:
                        transform_collectors[t.entity_path] = ColumnCollector(t.entity_path, rr.Transform3D)
                    transform_collectors[t.entity_path].append(
                        indexes={"message_log_time": t.log_time_ns, "message_publish_time": t.publish_time_ns},
                        translation=t.translation,
                        quaternion=rr.Quaternion(xyzw=t.quaternion),
                        parent_frame=t.parent_frame,
                        child_frame=t.child_frame,
                    )
                continue

            # Fallback: any unhandled message as DynamicArchetype
            archetype_name, entity_path, components = implicit_collect(msg)
            if entity_path not in dynamic_collectors:
                dynamic_collectors[entity_path] = ColumnCollector(
                    entity_path, rr.DynamicArchetype, archetype_name=archetype_name
                )
            dynamic_collectors[entity_path].append(
                indexes={"message_log_time": msg.log_time_ns, "message_publish_time": msg.publish_time_ns},
                **components,
            )

    # Send all collected time-series data using columnar API
    send_collected_columns(rec, video_collectors, transform_collectors, dynamic_collectors)
