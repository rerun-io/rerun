"""
Demo for table blueprints & segment previews.

Table blueprints allow configuring table layouts and use segment previews.

**TODO(#12745, #12746): This feature is experimental.**

Each row can reference a recording via a URI column. The viewer loads those recordings
on demand and renders them through the registered blueprint's view definition. A
preview column uses `TableCellKind.Preview` on its layout-specific `TableColumn` and stores its views
in `TableColumnPreview` on the same layout-specific blueprint entity. `PreviewsConfig` stores the
timeline shared by all preview columns.

The demo also includes a boolean `marker_flag` column. Its `TableColumn` uses the
`Flag` cell kind with editing enabled, and `CardLayout` includes it as a field.
Toggling the flag updates the visible table immediately and upserts the new boolean value back to
the server using the `rerun:is_table_index` column as the row key.

For testing you can use this droid rrd dataset:
https://huggingface.co/datasets/rerun/droid_sample/tree/main

Usage:
    table_blueprints
    table_blueprints /path/to/dataset
    table_blueprints --target dataset
    table_blueprints --target both
    table_blueprints --write-blueprints-only --blueprint-dir /tmp/table-blueprints
    table_blueprints <dataset-name> --url rerun+https://… --blueprint-uri-base s3://bucket/table-blueprints/

`--target` selects what the blueprints are applied to:
- `tables`: create the demo tables, each with its own table blueprint.
- `dataset`: register a blueprint on the dataset's own segment table (no tables created).
- `both` (default): do both.

Without `--url`, this starts a temporary local Rerun server for the given directory of
`.rrd` files. With `--url`, this connects as a client to an existing Rerun server or
catalog and expects `dataset` to be the remote dataset name.
Remote registration requires `--blueprint-uri-base` pointing at a server-visible
location containing the `.rbl` files written by this script.
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any, NamedTuple

import pyarrow as pa

import rerun as rr
import rerun.blueprint as rrb
from rerun import bindings
from rerun.recording_stream import RecordingStream
from rerun.server import Server


def save_table_blueprint(
    path: Path,
    *views: rrb.View,
    preview_column: str,
    flag_column: str | None = None,
    card_title_column: str | None = None,
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
    preview_column:
        Names the column whose values are `rerun://` recording URIs.
        The viewer loads those recordings and renders inline previews.
    flag_column:
        If set, names the boolean column used for flag/annotation toggles.
        The column must exist in the table schema.
    card_title_column:
        If set, names the column to use as card titles.
        If unset, the first visible string column is used.
    timeline:
        If set, selects the timeline used by the previews.

    """
    if not views:
        raise ValueError("A table preview requires at least one view")

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

        escaped_preview_column = rr.escape_entity_path_part(preview_column)
        table_preview_path = f"/table/layouts/table/columns/{escaped_preview_column}"
        card_preview_path = f"/table/layouts/cards/fields/{escaped_preview_column}"
        for preview_path in [table_preview_path, card_preview_path]:
            blueprint_stream.log(
                preview_path,
                rrb.experimental.TableColumn(
                    cell_kind=rrb.components.TableCellKind.Preview,
                ),
            )
            blueprint_stream.log(
                preview_path,
                rrb.experimental.TableColumnPreview(
                    views=[view.blueprint_path() for view in views],
                ),
            )

        card_fields = [preview_column]
        if flag_column is not None:
            card_fields.append(flag_column)
            blueprint_stream.log(
                f"/table/layouts/table/columns/{rr.escape_entity_path_part(flag_column)}",
                rrb.experimental.TableColumn(
                    editable=True,
                    cell_kind=rrb.components.TableCellKind.Flag,
                ),
            )
            blueprint_stream.log(
                f"/table/layouts/cards/fields/{rr.escape_entity_path_part(flag_column)}",
                rrb.experimental.TableColumn(
                    editable=True,
                    cell_kind=rrb.components.TableCellKind.Flag,
                ),
            )

        blueprint_stream.log(
            "/table",
            rrb.experimental.PreviewsConfig(timeline=timeline),
        )
        blueprint_stream.log(
            "/table/layouts/table",
            rrb.experimental.TableLayout(column_order=[preview_column]),
        )
        blueprint_stream.log(
            "/table/layouts/cards",
            rrb.experimental.CardLayout(
                field_order=card_fields,
                title=card_title_column,
                link=preview_column,
            ),
        )


# ---------------------------------------------------------------------------
# Dataset-specific customization
# ---------------------------------------------------------------------------

DEFAULT_LOCAL_DATASET = Path(__file__).resolve().parents[3] / "tests/assets/rrd/sample_5"
MARKER_FLAG_COLUMN = "marker_flag"
SEGMENT_RECORDING_LINK_COLUMN = "recording link"
SEGMENT_TABLE_BLUEPRINT_NAME = "segment_table"
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


class PreviewViews(NamedTuple):
    """The views shared by the table and segment-table blueprints."""

    plot: rrb.TimeSeriesView
    spatial_3d: rrb.Spatial3DView
    spatial_2d: rrb.Spatial2DView


def setup_preview_views() -> PreviewViews:
    """
    Build all views used by the demo blueprints.

    PLEASE EDIT THIS for your dataset: view origins, contents, target frame, and excluded paths.
    """
    return PreviewViews(
        plot=rrb.TimeSeriesView(
            origin="/observation/joint_positions",
            plot_legend=rrb.PlotLegend(visible=False),
        ),
        spatial_3d=rrb.Spatial3DView(
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
        ),
        spatial_2d=rrb.Spatial2DView(
            contents=["+ /camera/wrist/**"],
        ),
    )


def make_dataset_blueprints(blueprint_dir: Path) -> dict[str, Path]:
    """
    Write the table blueprints used by this demo to `blueprint_dir` and return their paths by name.

    These target the demo *tables* created by this script, whose schema has `recording_uri`,
    `marker_flag`, and `uuid` columns. For the dataset's own segment table, see
    `make_segment_table_blueprint`.

    PLEASE EDIT THIS for your dataset. In particular, update:
    - `card_title_column` to a string column that exists in your copied properties.
    - `timeline` to the timeline used by your recordings.
    """
    common_bp_kwargs = {
        "preview_column": "recording_uri",
        "flag_column": MARKER_FLAG_COLUMN,
        "card_title_column": "uuid",
        "timeline": "real_time",
    }

    views = setup_preview_views()

    blueprint_dir.mkdir(parents=True, exist_ok=True)
    paths = {
        name: blueprint_dir / f"{name}.rbl" for name in ("previews_plot", "previews_3d_only", "previews_3d_and_2d")
    }

    save_table_blueprint(paths["previews_plot"], views.plot, **common_bp_kwargs)
    save_table_blueprint(paths["previews_3d_only"], views.spatial_3d, **common_bp_kwargs)
    save_table_blueprint(paths["previews_3d_and_2d"], views.spatial_3d, views.spatial_2d, **common_bp_kwargs)

    return paths


def make_segment_table_blueprint(blueprint_dir: Path) -> Path:
    """
    Write the blueprint used for the dataset's own segment table and return its path.

    Unlike the table blueprints, this targets the dataset's native segment table and uses its
    generated `recording link` column for previews. Segment tables have no demo flag column.

    PLEASE EDIT THIS for your dataset. By default it uses the combined 3D & 2D views and the
    `real_time` timeline; adjust the views (via `setup_preview_views`) and timeline to match your
    recordings.
    """
    blueprint_dir.mkdir(parents=True, exist_ok=True)
    path = blueprint_dir / f"{SEGMENT_TABLE_BLUEPRINT_NAME}.rbl"

    views = setup_preview_views()
    save_table_blueprint(
        path,
        views.spatial_3d,
        views.spatial_2d,
        preview_column=SEGMENT_RECORDING_LINK_COLUMN,
        timeline="real_time",
    )

    return path


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


def create_demo_tables(
    client: rr.catalog.CatalogClient,
    dataset: rr.catalog.DatasetEntry,
    dataset_name: str,
    *,
    blueprint_dir: Path,
    blueprint_uri_base: str | None,
) -> None:
    """Create one demo table per table blueprint, populated from the dataset's segment properties."""
    _, segment_uris, props = query_segment_data(dataset)
    print(f"Using {len(segment_uris)} segments from dataset '{dataset_name}'")

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


def apply_segment_table_blueprint(
    dataset: rr.catalog.DatasetEntry,
    *,
    blueprint_dir: Path,
    blueprint_uri_base: str | None,
) -> None:
    """Register the segment-table blueprint on the dataset's own segment table."""
    path = make_segment_table_blueprint(blueprint_dir)
    uri = blueprint_uri(SEGMENT_TABLE_BLUEPRINT_NAME, path, blueprint_uri_base)
    dataset.register_blueprint(uri, segment_table=True)
    print(f"  segment table: registered blueprint {uri}")


def run_with_client(
    client: rr.catalog.CatalogClient,
    dataset_name: str,
    *,
    target: str,
    blueprint_dir: Path,
    blueprint_uri_base: str | None,
) -> None:
    """Create demo tables and/or register a blueprint on the dataset's segment table, per `target`."""
    dataset = client.get_dataset(dataset_name)

    if target in ("tables", "both"):
        create_demo_tables(
            client,
            dataset,
            dataset_name,
            blueprint_dir=blueprint_dir,
            blueprint_uri_base=blueprint_uri_base,
        )

    if target in ("dataset", "both"):
        apply_segment_table_blueprint(
            dataset,
            blueprint_dir=blueprint_dir,
            blueprint_uri_base=blueprint_uri_base,
        )


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
        "--target",
        choices=("tables", "dataset", "both"),
        default="both",
        help=(
            "What to apply blueprints to:\n"
            "* 'tables' creates the demo tables\n"
            "* 'dataset' registers a blueprint on the dataset's own segment table\n"
            "* 'both' (default) does both."
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
        make_segment_table_blueprint(args.blueprint_dir)
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
            target=args.target,
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
                target=args.target,
                blueprint_dir=args.blueprint_dir,
                blueprint_uri_base=args.blueprint_uri_base,
            )
            input("Press Enter to stop the server…")


if __name__ == "__main__":
    main()
