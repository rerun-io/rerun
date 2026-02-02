"""
Convert custom MCAP Protobuf messages to Rerun format.

This example shows how to read MCAP files containing custom Protobuf messages
(not Foxglove schemas) and convert them to Rerun archetypes. It demonstrates:
- Converting transform messages
- Converting camera calibration to Pinhole
- Converting compressed video streams
- Using DynamicArchetype as a fallback for arbitrary protobuf messages
"""

from __future__ import annotations

import argparse

from dataclasses import dataclass
from typing import Any

from google.protobuf.json_format import MessageToDict
from mcap.reader import make_reader
from mcap_protobuf.decoder import DecoderFactory
import numpy as np
import rerun as rr


@dataclass
class McapMessage:
    """Wrapper for decoded MCAP message data."""

    topic: str
    log_time_ns: int
    publish_time_ns: int
    proto_msg: Any


# Track static data we've already logged (to avoid duplicate static warnings)
_logged_static_transforms: set[tuple[str, str]] = set()
_logged_static_calibrations: set[str] = set()


def convert_timestamp(secs: int, nanos: int) -> np.datetime64:
    """Convert ROS2 timestamp to nanoseconds since epoch."""
    epoch_nanos = secs * 1_000_000_000 + nanos
    return np.datetime64(epoch_nanos, "ns")


def set_mcap_message_times(rec: rr.RecordingStream, msg: McapMessage) -> None:
    """
    Set both MCAP message timestamps on the recording stream.

    log_time_ns: when the message was logged by the recorder
    publish_time_ns: when the message was published
    """
    rec.set_time(timeline="message_log_time", timestamp=np.datetime64(msg.log_time_ns, "ns"))
    rec.set_time(timeline="message_publish_time", timestamp=np.datetime64(msg.publish_time_ns, "ns"))


def transform_msg(rec: rr.RecordingStream, msg: McapMessage) -> bool:
    """Convert FrameTransforms messages to Rerun transforms."""
    if msg.proto_msg.DESCRIPTOR.name != "FrameTransforms":
        return False

    # Static transform topics: transforms_static, etc.
    static_topics = {"transforms_static"}
    is_static = msg.topic in static_topics

    for transform in msg.proto_msg.transforms:
        parent = transform.parent_frame_id
        child = transform.child_frame_id

        rr_transform = rr.Transform3D(
            translation=(transform.translation.x, transform.translation.y, transform.translation.z),
            quaternion=rr.Quaternion(
                xyzw=[transform.rotation.x, transform.rotation.y, transform.rotation.z, transform.rotation.w]
            ),
            parent_frame=parent,
            child_frame=child,
        )

        entity_path = f"transforms/{child}"
        if is_static:
            key = (parent, child)
            if key in _logged_static_transforms:
                continue
            _logged_static_transforms.add(key)
            rec.log(entity_path, rr_transform, static=True)
        else:
            set_mcap_message_times(rec, msg)
            rec.log(entity_path, rr_transform)
    return True


def camera_calibration(rec: rr.RecordingStream, msg: McapMessage) -> bool:
    """Convert CameraCalibration messages to Rerun Pinhole."""
    if msg.proto_msg.DESCRIPTOR.name != "CameraCalibration":
        return False

    if msg.topic in _logged_static_calibrations:
        return True
    _logged_static_calibrations.add(msg.topic)

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


def compressed_video(rec: rr.RecordingStream, msg: McapMessage) -> bool:
    """Convert CompressedVideo messages to Rerun VideoStream."""
    if msg.proto_msg.DESCRIPTOR.name != "CompressedVideo":
        return False

    video_blob = rr.VideoStream(
        codec=msg.proto_msg.format,
        sample=msg.proto_msg.data,
    )
    set_mcap_message_times(rec, msg)
    rec.log(msg.topic, video_blob)
    return True


def implicit_convert(rec: rr.RecordingStream, msg: McapMessage) -> bool:
    """Fallback converter: any protobuf message -> DynamicArchetype."""
    contents = MessageToDict(
        msg.proto_msg,
        preserving_proto_field_name=True,
        always_print_fields_with_no_presence=True,
    )

    try:
        dynamic_archetype = rr.DynamicArchetype(
            archetype=msg.proto_msg.DESCRIPTOR.full_name,
            components=contents,
        )
    except Exception as e:
        raise ValueError(f"{msg.proto_msg.DESCRIPTOR.full_name} {contents}") from e

    rec.log(msg.topic, dynamic_archetype, strict=True)
    return True


# --- Main execution ---

parser = argparse.ArgumentParser(description="Convert MCAP Protobuf messages to Rerun format.")
parser.add_argument("mcap_file", help="Path to the MCAP file to convert")
args = parser.parse_args()

path_to_mcap = args.mcap_file

with rr.RecordingStream("rerun_example_convert_mcap_protobuf") as rec:
    rec.save("convert_mcap_protobuf.rrd")

    # Connect the viewer's root to the "world" frame.
    rec.log("/", rr.CoordinateFrame("world"), static=True)

    with open(path_to_mcap, "rb") as f:
        reader = make_reader(f, decoder_factories=[DecoderFactory()])
        for _schema, channel, message, proto_msg in reader.iter_decoded_messages():
            msg = McapMessage(
                topic=channel.topic,
                log_time_ns=message.log_time,
                publish_time_ns=message.publish_time,
                proto_msg=proto_msg,
            )
            if camera_calibration(rec, msg):
                continue
            if compressed_video(rec, msg):
                continue
            if transform_msg(rec, msg):
                continue
            if implicit_convert(rec, msg):
                continue
            print(f"Unhandled message on topic {msg.topic} of type {msg.proto_msg.DESCRIPTOR.name}")
