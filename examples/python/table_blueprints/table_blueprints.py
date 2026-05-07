"""
Demo for table blueprints & segment previews.

Table blueprints allow configuring table layouts and use segment previews.

**TODO(#12745, #12746): This feature is experimental.** Enable it in the viewer under Settings > Experimental > Grid view as well as Table blueprints.

Each row can reference a recording via a URI column. The viewer loads those recordings
on demand and renders them through the embedded blueprint's view definition.

Usage:
    table_blueprints
    table_blueprints /path/to/dataset
    table_blueprints <dataset-name> --url rerun+https://…

Without `--url`, this starts a temporary local Rerun server for the given directory of
`.rrd` files. With `--url`, this connects as a client to an existing Rerun server or
catalog and expects `dataset` to be the remote dataset name.
"""

from __future__ import annotations

import argparse
import base64
from pathlib import Path
from typing import Any

import pyarrow as pa

import rerun as rr
import rerun.blueprint as rrb
from rerun import bindings
from rerun.recording_stream import RecordingStream
from rerun.server import Server


def make_table_blueprint(
    *views: rrb.View,
    segment_preview_column: str | None = None,
    flag_column: str | None = None,
    grid_view_card_title: str | None = None,
    timeline: str | None = None,
) -> str:
    """
    Serialize one or more views into a base64-encoded blueprint for table schema metadata.

    Parameters
    ----------
    *views:
        One or more view definitions to embed (e.g. `Spatial3DView`, `TimeSeriesView`).
    segment_preview_column:
        If set, names the column whose values are `rerun://` recording URIs.
        The viewer will load those recordings and render inline previews.
    flag_column:
        If set, names the boolean column used for flag/annotation toggles.
        The column must exist in the table schema.
    grid_view_card_title:
        If set, names the column to use as card titles in grid view.
        If unset, the first visible string column is used.
    timeline:
        If set, configures the time panel to display this timeline.

    Returns
    -------
    str
        `"base64:<data>"` string suitable for Arrow schema metadata.

    """
    blueprint = rrb.Blueprint(*views)

    blueprint_stream = RecordingStream._from_native(
        bindings.new_blueprint(
            application_id="embedded",
            make_default=False,
            make_thread_default=False,
            default_enabled=True,
        ),
    )
    blueprint_stream.set_time("blueprint", sequence=0)
    blueprint._log_to_stream(blueprint_stream)

    table_blueprint_kwargs = {}
    if segment_preview_column is not None:
        table_blueprint_kwargs["segment_preview_column"] = segment_preview_column
    if flag_column is not None:
        table_blueprint_kwargs["flag_column"] = flag_column
    if grid_view_card_title is not None:
        table_blueprint_kwargs["grid_view_card_title"] = grid_view_card_title
    if table_blueprint_kwargs:
        blueprint_stream.log(
            "/table",
            rrb.experimental.TableBlueprint(**table_blueprint_kwargs),
        )

    if timeline is not None:
        rrb.TimePanel(timeline=timeline)._log_to_stream(blueprint_stream)

    rbl_bytes = blueprint_stream.memory_recording().drain_as_bytes()
    return "base64:" + base64.b64encode(rbl_bytes).decode("ascii")


# ---------------------------------------------------------------------------
# Dataset-specific customization
# ---------------------------------------------------------------------------

DEFAULT_LOCAL_DATASET = Path(__file__).resolve().parents[3] / "tests/assets/rrd/sample_5"
MARKER_FLAG_COLUMN = "marker_flag"
TABLE_BLUEPRINT_METADATA_KEY = b"rerun:table_blueprint"

PropertyColumn = tuple[str, pa.Field, list[Any]]

# Please edit the functions in this section to match your own dataset.
# The defaults below are geared towards RRDs from the DROID dataset and its schema,
# timelines, entity paths, and coordinate frames; they are intended as a starting point only.


def extract_dataset_property_columns(seg_arrow: pa.Table, num_segments: int) -> list[PropertyColumn]:
    """
    Pick which segment-table columns should be copied into the demo tables.

    PLEASE EDIT THIS for your dataset. The default implementation looks for
    columns named `property:episode:*` and strips that prefix.
    """
    episode_prefix = "property:episode:"
    props: list[PropertyColumn] = []
    for field in seg_arrow.schema:
        if field.name.startswith(episode_prefix):
            original_name = field.name
            short_name = original_name[len(episode_prefix) :]
            values = seg_arrow.column(original_name).to_pylist()[:num_segments]
            props.append((short_name, pa.field(short_name, field.type, field.nullable), values))

    return props


def make_dataset_blueprints() -> dict[str, str]:
    """
    Create the table blueprints used by this demo.

    PLEASE EDIT THIS for your dataset. In particular, update:
    - `grid_view_card_title` to a string column that exists in your copied properties.
    - `timeline` to the timeline used by your recordings.
    - view origins, contents, target frame, and excluded paths.
    """
    common_bp_kwargs = {
        "segment_preview_column": "recording_uri",
        "flag_column": MARKER_FLAG_COLUMN,
        "grid_view_card_title": "uuid",
        "timeline": "real_time",
    }

    spatial_3d_view = rrb.Spatial3DView(
        contents=[
            "+ /**",
            "- /camera/**",
            "- /**/collision_0/**",
            "- /thumbnail/**",
        ],
        spatial_information=rrb.SpatialInformation(
            target_frame="panda_link0",
        ),
        background=rrb.Background(
            color=[0.1, 0.1, 0.1, 1.0],
        ),
    )
    spatial_2d_view = rrb.Spatial2DView(
        contents=["+ /camera/wrist/**"],
    )

    return {
        "previews_plot": make_table_blueprint(
            rrb.TimeSeriesView(
                origin="/observation/joint_positions",
                plot_legend=rrb.PlotLegend(visible=False),
            ),
            **common_bp_kwargs,
        ),
        "previews_3d_only": make_table_blueprint(spatial_3d_view, **common_bp_kwargs),
        "previews_3d_and_2d": make_table_blueprint(spatial_3d_view, spatial_2d_view, **common_bp_kwargs),
    }


# ---------------------------------------------------------------------------
# Generic demo plumbing: start a local server, query segments, and create tables.
# ---------------------------------------------------------------------------


def query_segment_data(
    dataset: rr.catalog.DatasetEntry,
) -> tuple[list[str], list[str], list[PropertyColumn]]:
    """
    Query segment table and return (segment_ids, segment_uris, property_columns).

    Returns all entries from the segment table.
    """
    seg_df = dataset.segment_table()
    seg_arrow = pa.Table.from_batches(seg_df.collect())

    segment_ids = seg_arrow.column("rerun_segment_id").to_pylist()
    n = len(segment_ids)
    segment_uris = [dataset.segment_url(sid) for sid in segment_ids]
    props = extract_dataset_property_columns(seg_arrow, n)

    return segment_ids, segment_uris, props


def create_table_with_blueprint(
    client: rr.catalog.CatalogClient,
    *,
    table_name: str,
    blueprint_str: str,
    segment_uris: list[str],
    property_columns: list[PropertyColumn],
) -> rr.catalog.TableEntry:
    """Create a table with the given blueprint and segment data."""
    n = len(segment_uris)

    fields: list[pa.Field] = [
        pa.field("id", pa.int64(), metadata={rr.SORBET_IS_TABLE_INDEX: "true"}),
        pa.field("recording_uri", pa.utf8()),
    ]
    data: dict[str, list[Any]] = {
        "id": list(range(n)),
        "recording_uri": segment_uris,
    }

    for short_name, field, values in property_columns:
        fields.append(field)
        data[short_name] = values

    fields.append(pa.field(MARKER_FLAG_COLUMN, pa.bool_()))
    data[MARKER_FLAG_COLUMN] = [False] * n

    schema = pa.schema(fields, metadata={TABLE_BLUEPRINT_METADATA_KEY: blueprint_str.encode("ascii")})
    table = client.create_table(table_name, schema)
    table.append(**data)
    return table


def run_with_client(client: rr.catalog.CatalogClient, dataset_name: str) -> None:
    """Create tables with different view blueprints from a dataset's real properties."""
    dataset = client.get_dataset(dataset_name)
    segment_ids, segment_uris, props = query_segment_data(dataset)
    print(f"Using {len(segment_ids)} segments from dataset '{dataset_name}'")

    blueprints = make_dataset_blueprints()

    existing_table_names = set(client.table_names())
    for name, bp in blueprints.items():
        if name in existing_table_names:
            client.get_table(name).delete()
            print(f"  {name}: deleted existing table")
        table = create_table_with_blueprint(
            client,
            table_name=name,
            blueprint_str=bp,
            segment_uris=segment_uris,
            property_columns=props,
        )
        bp_size = len(table.arrow_schema().metadata.get(TABLE_BLUEPRINT_METADATA_KEY))
        print(f"  {name}: blueprint {bp_size} bytes")


def main() -> None:
    parser = argparse.ArgumentParser(description="Create table-blueprint demo tables.")
    parser.add_argument(
        "dataset",
        nargs="?",
        help=(
            "Without --url: local dataset directory to serve. "
            "With --url: remote dataset name to look up. "
            f"Defaults to {DEFAULT_LOCAL_DATASET} in local server mode."
        ),
    )

    connection_group = parser.add_mutually_exclusive_group()
    connection_group.add_argument("--port", type=int, default=None, help="Port for local server mode.")
    connection_group.add_argument("--url", help="Remote server/catalog URL for client mode.")

    args = parser.parse_args()

    if args.url is not None:
        if args.dataset is None:
            parser.error("Provide a remote dataset name when using --url")
        client = rr.catalog.CatalogClient(args.url)
        run_with_client(client, dataset_name=args.dataset)
    else:
        local_dataset = args.dataset or str(DEFAULT_LOCAL_DATASET)
        with Server(port=args.port, datasets={"local": local_dataset}) as srv:
            print(srv.url())
            client = srv.client()
            run_with_client(client, dataset_name="local")
            input("Press Enter to stop the server…")


if __name__ == "__main__":
    main()
