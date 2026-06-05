"""
Demo for table blueprints & segment previews.

Table blueprints allow configuring table layouts and use segment previews.

**TODO(#12745, #12746): This feature is experimental.** Enable it in the
viewer under Settings > Experimental > Table cards and blueprints.

Each row can reference a recording via a URI column. The viewer loads those recordings
on demand and renders them through the registered blueprint's view definition.

The demo also includes a boolean `marker_flag` column and points the registered table
blueprint at it. The Viewer uses that column as the per-row flag state: toggling a
card's flag updates the visible table immediately and upserts the new boolean value
back to the server using the `rerun:is_table_index` column as the row key.

For testing you can use this droid rrd dataset:
https://huggingface.co/datasets/rerun/droid_sample/tree/main

Usage:
    table_blueprints
    table_blueprints /path/to/dataset
    table_blueprints --write-blueprints-only --blueprint-dir /tmp/table-blueprints
    table_blueprints <dataset-name> --url rerun+https://… --blueprint-uri-base s3://bucket/table-blueprints/

Without `--url`, this starts a temporary local Rerun server for the given directory of
`.rrd` files. With `--url`, this connects as a client to an existing Rerun server or
catalog and expects `dataset` to be the remote dataset name.
Remote registration requires `--blueprint-uri-base` pointing at a server-visible
location containing the `.rbl` files written by this script.
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any

import pyarrow as pa

import rerun as rr
import rerun.blueprint as rrb
from rerun import bindings
from rerun.recording_stream import RecordingStream
from rerun.server import Server


def save_table_blueprint(
    path: Path,
    *views: rrb.View,
    segment_preview_column: str | None = None,
    flag_column: str | None = None,
    grid_view_card_title: str | None = None,
    timeline: str | None = None,
) -> None:
    """
    Write a table blueprint with one or more views into a `.rbl` file.

    Parameters
    ----------
    path:
        File path to write the serialized `.rbl` blueprint to.
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

    """
    blueprint = rrb.Blueprint(*views)

    with RecordingStream._from_native(
        bindings.new_blueprint(
            application_id="embedded",
            make_default=False,
            make_thread_default=False,
            default_enabled=True,
        ),
    ) as blueprint_stream:
        blueprint_stream.save(str(path))
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


# ---------------------------------------------------------------------------
# Dataset-specific customization
# ---------------------------------------------------------------------------

DEFAULT_LOCAL_DATASET = Path(__file__).resolve().parents[3] / "tests/assets/rrd/sample_5"
MARKER_FLAG_COLUMN = "marker_flag"
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


def make_dataset_blueprints(blueprint_dir: Path) -> dict[str, Path]:
    """
    Write the table blueprints used by this demo to `blueprint_dir` and return their paths by name.

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

    blueprint_dir.mkdir(parents=True, exist_ok=True)
    paths = {
        name: blueprint_dir / f"{name}.rbl" for name in ("previews_plot", "previews_3d_only", "previews_3d_and_2d")
    }

    save_table_blueprint(
        paths["previews_plot"],
        rrb.TimeSeriesView(
            origin="/observation/joint_positions",
            plot_legend=rrb.PlotLegend(visible=False),
        ),
        **common_bp_kwargs,
    )
    save_table_blueprint(paths["previews_3d_only"], spatial_3d_view, **common_bp_kwargs)
    save_table_blueprint(paths["previews_3d_and_2d"], spatial_3d_view, spatial_2d_view, **common_bp_kwargs)

    return paths


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


def create_table(
    client: rr.catalog.CatalogClient,
    *,
    table_name: str,
    segment_uris: list[str],
    property_columns: list[PropertyColumn],
) -> rr.catalog.TableEntry:
    """Create a table with the given segment data."""
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

    schema = pa.schema(fields)
    table = client.create_table(table_name, schema)
    table.append(**data)
    return table


def blueprint_uri(name: str, local_path: Path, blueprint_uri_base: str | None) -> str:
    """Return the URI to register for a blueprint."""
    if blueprint_uri_base is None:
        return local_path.absolute().as_uri()
    return blueprint_uri_base.rstrip("/") + f"/{name}.rbl"


def run_with_client(
    client: rr.catalog.CatalogClient,
    dataset_name: str,
    *,
    blueprint_dir: Path,
    blueprint_uri_base: str | None,
) -> None:
    """Create tables with different view blueprints from a dataset's real properties."""
    dataset = client.get_dataset(dataset_name)
    segment_ids, segment_uris, props = query_segment_data(dataset)
    print(f"Using {len(segment_ids)} segments from dataset '{dataset_name}'")

    blueprint_paths = make_dataset_blueprints(blueprint_dir)

    existing_table_names = set(client.table_names())
    for name in blueprint_paths:
        if name in existing_table_names:
            client.get_table(name).delete()
            print(f"  {name}: deleted existing table")
        table = create_table(
            client,
            table_name=name,
            segment_uris=segment_uris,
            property_columns=props,
        )
        uri = blueprint_uri(name, blueprint_paths[name], blueprint_uri_base)
        table.register_blueprint(uri)
        print(f"  {name}: registered table blueprint {uri}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Create table-blueprint demo tables.")
    parser.add_argument(
        "dataset",
        nargs="?",
        help=(f"Local dataset directory to serve. Defaults to {DEFAULT_LOCAL_DATASET}."),
    )

    connection_group = parser.add_mutually_exclusive_group()
    connection_group.add_argument("--port", type=int, default=None, help="Port for local server mode.")
    connection_group.add_argument("--url", help="Remote server/catalog URL for client mode.")
    parser.add_argument(
        "--blueprint-dir",
        type=Path,
        default=Path.cwd(),
        help="Directory where generated .rbl table blueprints are written.",
    )
    parser.add_argument(
        "--blueprint-uri-base",
        help=(
            "Server-visible URI prefix used when registering generated .rbl files. "
            "Required with --url unless --write-blueprints-only is used."
        ),
    )
    parser.add_argument(
        "--write-blueprints-only",
        action="store_true",
        help="Only write generated .rbl files to --blueprint-dir, then exit.",
    )

    args = parser.parse_args()

    if args.write_blueprints_only:
        make_dataset_blueprints(args.blueprint_dir)
        return

    if args.url is not None:
        if args.dataset is None:
            parser.error("Provide a remote dataset name when using --url")
        if args.blueprint_uri_base is None:
            parser.error("Provide --blueprint-uri-base with --url after uploading the generated .rbl files")
        client = rr.catalog.CatalogClient(args.url)
        run_with_client(
            client,
            dataset_name=args.dataset,
            blueprint_dir=args.blueprint_dir,
            blueprint_uri_base=args.blueprint_uri_base,
        )
    else:
        local_dataset = args.dataset or str(DEFAULT_LOCAL_DATASET)
        with Server(port=args.port, datasets={"local": local_dataset}) as srv:
            print(srv.url())
            client = srv.client()
            run_with_client(
                client,
                dataset_name="local",
                blueprint_dir=args.blueprint_dir,
                blueprint_uri_base=args.blueprint_uri_base,
            )
            input("Press Enter to stop the server…")


if __name__ == "__main__":
    main()
