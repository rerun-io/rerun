from __future__ import annotations

from typing import TYPE_CHECKING

from datafusion import col
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    import pyarrow as pa
    import rerun_draft as rr


def test_dataset_view_filter_segments(populated_client: rr.catalog.CatalogClient) -> None:
    orig_ds = populated_client.get_dataset_entry(name="basic_dataset")
    meta = populated_client.get_table(name="basic_dataset_metadata")

    simple_filt = orig_ds.filter_dataset(segment_ids=["simple_recording_0"])

    assert sorted(simple_filt.segment_ids()) == inline_snapshot(["simple_recording_0"])

    assert str(
        simple_filt.segment_table(join_meta=meta)
        .drop("rerun_storage_urls", "rerun_last_updated_at")
        .sort("rerun_segment_id")
    ) == inline_snapshot("""\
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                              │
│ * version: 0.1.1                                                                                       │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌────────────────────┬───────────────────┬──────────────────┬──────────────────┬─────────────────────┐ │
│ │ rerun_segment_id   ┆ rerun_layer_names ┆ rerun_num_chunks ┆ rerun_size_bytes ┆ success             │ │
│ │ ---                ┆ ---               ┆ ---              ┆ ---              ┆ ---                 │ │
│ │ type: Utf8         ┆ type: List[Utf8]  ┆ type: u64        ┆ type: u64        ┆ type: nullable bool │ │
│ ╞════════════════════╪═══════════════════╪══════════════════╪══════════════════╪═════════════════════╡ │
│ │ simple_recording_0 ┆ [base]            ┆ 2                ┆ 1392             ┆ true                │ │
│ └────────────────────┴───────────────────┴──────────────────┴──────────────────┴─────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    good_segments = orig_ds.segment_table(join_meta=meta).fill_null(True, ["success"]).filter(col("success"))

    good_ds = orig_ds.filter_dataset(segment_ids=good_segments)

    assert sorted(good_ds.segment_ids()) == inline_snapshot(["simple_recording_0", "simple_recording_1"])

    assert str(
        good_ds.segment_table(join_meta=meta)
        .drop("rerun_storage_urls", "rerun_last_updated_at")
        .sort("rerun_segment_id")
    ) == inline_snapshot("""\
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                              │
│ * version: 0.1.1                                                                                       │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌────────────────────┬───────────────────┬──────────────────┬──────────────────┬─────────────────────┐ │
│ │ rerun_segment_id   ┆ rerun_layer_names ┆ rerun_num_chunks ┆ rerun_size_bytes ┆ success             │ │
│ │ ---                ┆ ---               ┆ ---              ┆ ---              ┆ ---                 │ │
│ │ type: Utf8         ┆ type: List[Utf8]  ┆ type: u64        ┆ type: u64        ┆ type: nullable bool │ │
│ ╞════════════════════╪═══════════════════╪══════════════════╪══════════════════╪═════════════════════╡ │
│ │ simple_recording_0 ┆ [base]            ┆ 2                ┆ 1392             ┆ true                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_1 ┆ [base]            ┆ 2                ┆ 1392             ┆ null                │ │
│ └────────────────────┴───────────────────┴──────────────────┴──────────────────┴─────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


def sorted_schema_str(schema: pa.Schema, with_metadata: bool = False) -> str:
    """A version of pa.Schema.__str__ that has stable field / metadata order."""

    # Iterate through every field in order. Print the field name and type,
    # then print its metadata in sorted order.
    lines = []
    for field in sorted(schema, key=lambda f: f.name):
        lines.append(f"{field.name}: {field.type}")
        if with_metadata and field.metadata:
            lines.append("  -- field metadata --")
            for key, value in sorted(field.metadata.items(), key=lambda kv: kv[0]):
                lines.append(f"  {key.decode('utf-8')}: '{value.decode('utf-8')}'")

    # Finally print the top-level schema metadata in sorted order.
    if with_metadata and schema.metadata:
        lines.append("-- schema metadata --")
        for key, value in sorted(schema.metadata.items(), key=lambda kv: kv[0]):
            lines.append(f"{key.decode('utf-8')}: '{value.decode('utf-8')}'")

    return "\n".join(lines)


def test_dataset_view_filter_entities(populated_client_complex: rr.catalog.CatalogClient) -> None:
    orig_ds = populated_client_complex.get_dataset_entry(name="complex_dataset")

    assert sorted_schema_str(orig_ds.arrow_schema()) == inline_snapshot("""\
/points:Points2D:colors: list<item: uint32>
/points:Points2D:positions: list<item: fixed_size_list<item: float not null>[2]>
/text:TextLog:text: list<item: string>
property:RecordingInfo:start_time: list<item: int64>
rerun.controls.RowId: fixed_size_binary[16]
timeline: timestamp[ns]\
""")

    entity_filt = orig_ds.filter_dataset(entity_paths=["/points"])

    assert sorted_schema_str(entity_filt.arrow_schema()) == inline_snapshot("""\
/points:Points2D:colors: list<item: uint32>
/points:Points2D:positions: list<item: fixed_size_list<item: float not null>[2]>
rerun.controls.RowId: fixed_size_binary[16]
timeline: timestamp[ns]\
""")


def test_dataset_view_dataframe(populated_client_complex: rr.catalog.CatalogClient) -> None:
    orig_ds = populated_client_complex.get_dataset_entry(name="complex_dataset")

    entity_filt = orig_ds.filter_dataset(
        entity_paths=["/text"], segment_ids=["complex_recording_0", "complex_recording_2"]
    )

    df = entity_filt.query(index="timeline").sort("rerun_segment_id")

    assert str(df) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                   │
│ * version: 0.1.1                                                                            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────┬──────────────────────────────┬────────────────────────────────────┐ │
│ │ rerun_segment_id    ┆ timeline                     ┆ /text:TextLog:text                 │ │
│ │ ---                 ┆ ---                          ┆ ---                                │ │
│ │ type: Utf8          ┆ type: nullable Timestamp(ns) ┆ type: nullable List[nullable Utf8] │ │
│ │                     ┆ index_name: timeline         ┆ archetype: TextLog                 │ │
│ │                     ┆ kind: index                  ┆ component: TextLog:text            │ │
│ │                     ┆                              ┆ component_type: Text               │ │
│ │                     ┆                              ┆ entity_path: /text                 │ │
│ │                     ┆                              ┆ kind: data                         │ │
│ ╞═════════════════════╪══════════════════════════════╪════════════════════════════════════╡ │
│ │ complex_recording_0 ┆ 2000-01-01T00:00:00          ┆ [Hello]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_0 ┆ 2000-01-01T00:00:02          ┆ [World]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_2 ┆ 2000-01-01T00:00:02          ┆ [Hello]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_2 ┆ 2000-01-01T00:00:04          ┆ [World]                            │ │
│ └─────────────────────┴──────────────────────────────┴────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────┘\
""")
