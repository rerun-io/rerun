"""
Configure table layouts and segment previews with Python blueprints.

Creates a table from DROID episode properties and registers blueprints that
show metadata and recording links in rows, or segment previews on cards.
Both the catalog table and the dataset's segment table use these layouts.
Also registers an editable-flag blueprint without making it the default.

Downloads and caches a recordings from rerun/droid_sample on
Hugging Face and serves them through a temporary local catalog.
Opens or reuses a Viewer and keeps the catalog alive until you press Enter.
Use --headless to start a Viewer without a window, or --no-viewer to exit
after registering the blueprints.
"""

from __future__ import annotations

import argparse
import os
import tempfile
import time
from dataclasses import replace
from pathlib import Path

import rerun as rr

# region: preview
import rerun.blueprint as rrb

preview = rrb.table.Column(
    "recording_uri",
    name="Recording",  # Give the column a nicer name.
    # Show a 3D view for recordings in this row.
    cell=rrb.table.PreviewCell(rrb.Spatial3DView()),
)
# endregion: preview

# region: layouts
table_layout = rrb.table.TableLayout(
    # Show these columns first.
    columns=[
        rrb.table.Column("recording_uri", name="Recording"),
        rrb.table.Column("uuid"),
        rrb.table.Column("current_task"),
    ],
)
card_layout = rrb.table.CardLayout(
    title="uuid",  # Use the uuid column as the card title.
    link="recording_uri",
    # Columns on the card layout are opt-in.
    fields=[preview, rrb.table.Column("current_task")],
)
blueprint = rrb.TableBlueprint(
    table_layout=table_layout,
    card_layout=card_layout,  # Cards become the default layout.
    previews_config=rrb.table.PreviewsConfig(timeline="real_time"),
)
# endregion: layouts

# region: flags
# Edit the `reviewed` column through flag buttons.
flag = rrb.table.Column("reviewed", editable=True, cell=rrb.table.FlagCell())
flag_blueprint = rrb.TableBlueprint(
    table_layout=rrb.table.TableLayout(columns=[flag, *table_layout.columns]),
    card_layout=rrb.table.CardLayout(
        title="uuid",
        link="recording_uri",
        fields=[flag, *card_layout.fields],
    ),
    previews_config=rrb.table.PreviewsConfig(timeline="real_time"),
)
# endregion: flags


def register_table(catalog_url: str) -> None:
    # region: register_table
    path = Path("table.rbl").resolve()
    blueprint.save("my_app", path)

    client = rr.catalog.CatalogClient(catalog_url)
    # Register a file the server can access.
    client.get_table("my_table").register_blueprint(path.as_uri())
    # endregion: register_table


def register_segments(client: rr.catalog.CatalogClient) -> None:
    # region: register_segments
    preview = rrb.table.Column(
        # Use the recording links provided by the segment table.
        "recording link",
        cell=rrb.table.PreviewCell(rrb.Spatial3DView()),
    )
    blueprint = rrb.TableBlueprint(
        table_layout=rrb.table.TableLayout(
            columns=[
                rrb.table.Column("recording link"),
            ],
        ),
        card_layout=rrb.table.CardLayout(
            title="property:episode:uuid",
            link="recording link",
            fields=[preview],
        ),
        previews_config=rrb.table.PreviewsConfig(timeline="real_time"),
    )
    path = Path("segments.rbl").resolve()
    blueprint.save("my_app", path)

    client.get_dataset("my_dataset").register_blueprint(
        path.as_uri(),
        # Apply to the segment table, not individual recordings.
        segment_table=True,
    )
    # endregion: register_segments


def register_remote(client: rr.catalog.CatalogClient) -> None:
    """Requires a blueprint uploaded to the server-accessible S3 URI."""
    # region: register_remote
    # Upload the .rbl to this URI first.
    client.get_table("my_table").register_blueprint(
        "s3://my-bucket/blueprints/table.rbl"
    )
    # endregion: register_remote


def save_screenshots(
    catalog_url: str,
    table: rr.catalog.TableEntry,
    directory: Path,
) -> None:
    """Capture screenshots after a fixed delay, for manual inspection."""
    from typing import Literal

    from rerun.experimental import ViewerClient

    directory.mkdir(parents=True, exist_ok=True)
    layouts: list[Literal["table", "cards"]] = ["table", "cards"]
    for suffix, config in [("", blueprint), ("_flags", flag_blueprint)]:
        for layout in layouts:
            name = f"{layout}{suffix}"
            path = Path(f"screenshot_{name}.rbl").resolve()
            replace(config, default_layout=layout).save("my_app", path)
            table.register_blueprint(path.as_uri())

            # An open viewer does not pick up default table blueprint changes,
            # so we have to re-open it each time.
            with ViewerClient.spawn(headless=True) as viewer:
                viewer.open_url(f"{catalog_url}/entry/{table.id}")
                # Previews load asynchronously after the table opens and
                # we don't have a way of waiting yet.
                time.sleep(5)

                screenshot = directory / f"table_blueprint_{name}.png"
                viewer.save_screenshot(str(screenshot))
                print(f"Saved {screenshot}", flush=True)


def run(
    rrd_paths: list[Path],
    *,
    screenshots: Path | None,
    headless: bool,
    open_viewer: bool,
) -> None:
    import pyarrow as pa

    from rerun.experimental import ViewerClient

    with rr.server.Server(datasets={"my_dataset": rrd_paths}) as server:
        client = server.client()
        dataset = client.get_dataset("my_dataset")
        segments = pa.Table.from_batches(dataset.segment_table().collect())
        segment_ids = segments.column("rerun_segment_id").to_pylist()
        uuids = segments.column("property:episode:uuid").to_pylist()
        tasks = segments.column("property:episode:current_task").to_pylist()
        schema = pa.schema([
            pa.field(
                "id", pa.int64(), metadata={rr.SORBET_IS_TABLE_INDEX: "true"}
            ),
            pa.field("recording_uri", pa.string()),
            pa.field("uuid", pa.string()),
            pa.field("current_task", pa.string()),
            pa.field("reviewed", pa.bool_()),
        ])
        table = client.create_table("my_table", schema)
        table.append(
            id=list(range(len(segment_ids))),
            recording_uri=[dataset.segment_url(sid) for sid in segment_ids],
            uuid=[uuid[0] for uuid in uuids],
            current_task=[task[0] for task in tasks],
            reviewed=[index % 2 == 0 for index in range(len(segment_ids))],
        )

        register_table(server.url())
        register_segments(client)

        flag_path = Path("flags.rbl").resolve()
        flag_blueprint.save("my_app", flag_path)
        table.register_blueprint(flag_path.as_uri(), set_default=False)

        if screenshots is not None:
            save_screenshots(server.url(), table, screenshots)
        elif open_viewer:
            print(f"Catalog: {server.url()}", flush=True)
            print("Open my_table or my_dataset to explore cards and previews.")
            with ViewerClient.spawn(
                headless=headless, detach_process=False
            ) as viewer:
                viewer.open_url(server.url())
                input("Press Enter to stop the example…")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--headless",
        action="store_true",
        help="Start the Viewer without a window; an existing Viewer is reused.",
    )
    parser.add_argument(
        "--no-viewer",
        action="store_true",
        help="Register blueprints without opening a Viewer.",
    )
    parser.add_argument(
        "--screenshots",
        type=Path,
        help="Capture table and card layouts, with and without editable flags.",
    )
    parser.add_argument(
        "--dataset",
        type=Path,
        help="Use a local RRD directory instead of downloading.",
    )
    parser.add_argument(
        "--num-recordings",
        type=int,
        default=20,
        help="Number of Hugging Face recordings to download (default: 20).",
    )
    args = parser.parse_args()
    if args.num_recordings < 1:
        parser.error("--num-recordings must be positive")
    if args.dataset is not None:
        rrd_paths = sorted(args.dataset.resolve().rglob("*.rrd"))
    else:
        from huggingface_hub import HfApi, snapshot_download

        repo_id = "rerun/droid_sample"
        files = sorted(
            name
            for name in HfApi().list_repo_files(repo_id, repo_type="dataset")
            if name.endswith(".rrd")
        )[: args.num_recordings]
        dataset_path = Path(
            snapshot_download(
                repo_id,
                repo_type="dataset",
                allow_patterns=files,
            )
        )
        rrd_paths = [dataset_path / name for name in files]
    if not rrd_paths:
        parser.error("No RRD recordings found")

    screenshots = (
        args.screenshots.resolve() if args.screenshots is not None else None
    )
    previous_directory = Path.cwd()
    with tempfile.TemporaryDirectory() as directory:
        try:
            os.chdir(directory)
            run(
                rrd_paths,
                screenshots=screenshots,
                headless=args.headless,
                open_viewer=not args.no_viewer,
            )
        finally:
            os.chdir(previous_directory)
