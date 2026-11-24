"""Register recordings in a catalog and query them."""

from __future__ import annotations

import tempfile
from pathlib import Path

import rerun as rr


def write_recording(path: Path, recording_id: str) -> str:
    with rr.RecordingStream(
        "rerun_example_use_catalog", recording_id=recording_id
    ) as recording:
        recording.save(path)
        for value in (0.1, 0.2, 0.3):
            recording.log("/obs", rr.Scalars(value))
    return path.resolve().as_uri()


def main() -> None:
    with (
        tempfile.TemporaryDirectory() as directory,
        rr.server.Server() as server,
    ):
        catalog_url = server.url()
        recording_uris = [
            write_recording(Path(directory) / f"{name}.rrd", name)
            for name in ("recording1", "recording2")
        ]

        use_catalog(catalog_url, recording_uris)


def use_catalog(catalog_url: str, recording_uris: list[str]) -> None:
    # region: prepare
    import rerun as rr

    # For Rerun Hub, e.g. "rerun://api.example.cloud.rerun.io"
    client = rr.catalog.CatalogClient(catalog_url)
    dataset = client.create_dataset("my_data")
    # For Rerun Hub, e.g. ["s3://my-rrd-files/recording1.rrd", …]
    dataset.register(recording_uris).wait()
    # endregion: prepare

    # region: query
    import datafusion as dfn

    import rerun as rr

    client = rr.catalog.CatalogClient(catalog_url)
    dataset = client.get_dataset("my_data")
    # `df` is a DataFusion dataframe
    df = dataset.filter_contents("/obs").reader(index="log_time")
    # Count observations in the recordings
    df.filter(dfn.col("/obs:Scalars:scalars").is_not_null()).count()
    # endregion: query

    assert df.count() == 6


if __name__ == "__main__":
    main()
