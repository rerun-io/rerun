"""Helper to write a dataframe to rrd(s)."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

import pyarrow as pa
import pyarrow.compute as pc
import rerun_bindings
from rerun_bindings import DatasetEntry

import rerun as rr
from rerun._baseclasses import ComponentDescriptor
from rerun.dataframe import ComponentColumnDescriptor, IndexColumnDescriptor


@dataclass(frozen=True)
class ColumnDesc:
    name: str
    entity_path: str
    archetype: str
    component: str
    is_static: bool
    component_type: str


def write_dataframe_to_rrd(dataset: DatasetEntry, output_dir: Path, partitions: list[str]) -> None:
    # This should be parallelized but flush doesn't work or global recording. We get a bunch of partial rrds
    # with ThreadPool(cpu_count()) as pool:
    #     results = []
    #     for partition in partitions[:2]:
    #         results.append(pool.apply_async(write_partition_to_rrd, args=(dataset, output_dir, partition)))
    #     for result in results:
    #         result.wait()
    for partition in partitions:
        write_partition_to_rrd(dataset, output_dir, partition)


def mark_as_scalars(contents: pa.Array, item: ComponentColumnDescriptor) -> tuple[pa.Array, str, ComponentDescriptor]:
    # TODO(nick): Check if we need to mangle the entity path?
    entity_path = str(Path(item.entity_path) / item.component.replace(":", "/"))
    component_content = contents.to_arrow_table()[item.name].combine_chunks()
    descriptor = ComponentDescriptor(
        component="Scalars:scalars",
        component_type="rerun.components.Scalar",
        archetype="rerun.archetypes.Scalars",
    )
    return component_content, entity_path, descriptor


def cast_to_scalars(contents: pa.Array, item: ComponentColumnDescriptor) -> tuple[pa.Array, str, ComponentDescriptor]:
    component_content, entity_path, descriptor = mark_as_scalars(contents, item)
    values_array = component_content.values
    # TODO(nick) logic to skip this if not needed instead of duplicate function hack
    float64_values = values_array
    if values_array.type != pa.list_(pa.float64()):
        float64_values = pc.cast(values_array, pa.float64())
        component_content = pa.ListArray.from_arrays(
            component_content.offsets, float64_values, type=pa.list_(pa.float64())
        )
    else:
        component_content = component_content.flatten()
    return component_content, entity_path, descriptor


def mark_as_video(contents: pa.Array, item: ComponentColumnDescriptor) -> tuple[pa.Array, str, ComponentDescriptor]:
    # TODO(nick): Check if we need to mangle the entity path?
    entity_path = str(Path(item.entity_path) / item.component.replace(":", "/"))
    component_content = contents.to_arrow_table()[item.name].combine_chunks()
    descriptor = ComponentDescriptor(
        component="VideoStream:sample",
        component_type="rerun.components.VideoSample",
        archetype="rerun.archetypes.VideoStream",
    )
    try:
        # Attempt 1
        # component_content = pa.UInt8Array.from_buffers(
        #     pa.uint8(),
        #     len(component_content),
        #     [None, component_content.buffers()[2]],
        # )
        # Attempt 2
        # component_content = component_content.flatten().cast(pa.uint8())
        # Attempt N + 1
        all_bytes = []
        for blob in component_content:
            blob_bytes = []
            for byte in blob[0].as_py():
                blob_bytes.append(byte)
            all_bytes.append([blob_bytes])
        component_content = pa.array(all_bytes, pa.list_(pa.list_(pa.uint8())))
        print(f"{component_content.type=}")
    except Exception as e:
        raise e
        raise ValueError(f"Could not convert {item.name} to UInt8Array: {component_content}") from e

    return component_content, entity_path, descriptor


def get_codec(contents: pa.Array, item: ComponentColumnDescriptor) -> tuple[pa.Array, str, ComponentDescriptor]:
    entity_path = str(Path(item.entity_path) / item.component.replace(":", "/").replace("format", "data"))
    component_content = contents.to_arrow_table()[item.name].combine_chunks()
    codec_descriptor = ComponentDescriptor(
        component="VideoStream:codec",
        component_type="rerun.components.VideoCodec",
        archetype="rerun.archetypes.VideoStream",
    )
    format = component_content[0][0].as_py().lower()
    print(f"{component_content=}")
    if "h264" in format:
        codec_content = pa.array([rr.components.VideoCodec.auto(format).value], type=pa.uint32())
    elif "h265" in format:
        codec_content = pa.array([rr.components.VideoCodec.auto(format).value], type=pa.uint32())
    else:
        raise ValueError(f"Could not infer codec from {item.name}")
    return codec_content, entity_path, codec_descriptor


def write_partition_to_rrd(dataset: DatasetEntry, output_dir: Path, partition: str) -> None:
    rec = rr.RecordingStream(
        application_id=dataset.name,
        recording_id=f"{dataset.name}_{partition}",
        send_properties=False,
    )
    output_dir.mkdir(parents=True, exist_ok=True)
    rec.save(output_dir / (partition + ".rrd"))
    index_column = None
    # print(f"{dataset.schema()=}")
    for item in dataset.schema():
        if isinstance(item, IndexColumnDescriptor):
            index_column = item.name
        else:
            assert isinstance(item, ComponentColumnDescriptor)  # Make mypy happy
            # TODO(nick) I guess this automatically gets added? I already disabled sending properties above
            # maybe a bug
            if "properties" in item.entity_path and item.component == "RecordingInfo:start_time":
                continue
            local_index_column = index_column
            if item.is_static:
                local_index_column = None
            contents = (
                dataset.dataframe_query_view(index=local_index_column, contents={item.entity_path: item.component})
                .filter_partition_id(partition)
                .df()
            )
            timelines = {}
            try:
                if not item.is_static:
                    indexes = contents.to_arrow_table()[local_index_column].combine_chunks()
                    timelines = {index_column: indexes}
            except Exception as e:
                raise ValueError(f"Could not load {item.name}") from e
            name = item.name
            entity_path = item.entity_path
            try:
                if False:
                    timelines = {}
                else:
                    descriptor = ComponentDescriptor(
                        component=item.component,
                        component_type=item.component_type,
                        archetype=item.archetype,
                    )
                    component_content = contents.to_arrow_table()[item.name].combine_chunks()
            except KeyError:
                print(f"Could not find {name} in arrow table. If it can't be queried it shouldn't be in schema :()")
                continue
            rerun_bindings.send_arrow_chunk(
                entity_path=entity_path,
                timelines=timelines,
                components={
                    descriptor: component_content,
                },
                recording=rec.inner,  # NOLINT
            )
    rec.flush()
