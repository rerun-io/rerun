from __future__ import annotations

from typing import TYPE_CHECKING, Literal

from rerun_bindings import LeRobotReaderInternal

from ..chunk import LazyChunkStream

if TYPE_CHECKING:
    from pathlib import Path


class LeRobotReader:
    """
    Read chunks from a LeRobot dataset, one episode at a time.

    The reader is a lightweight handle over the dataset directory: constructing it
    validates that the path is a v2 or v3 LeRobot dataset and loads its metadata.
    Enumerate episodes with `episodes()` and produce chunks with `stream(episode, ...)`.

    Parameters
    ----------
    path:
        Path to the LeRobot dataset directory (the one containing `meta/` and `data/`).

    Raises
    ------
    FileNotFoundError
        If `path` does not exist.
    ValueError
        If `path` is not a v2 or v3 LeRobot dataset.

    """

    _internal: LeRobotReaderInternal

    def __init__(self, path: str | Path) -> None:
        self._internal = LeRobotReaderInternal(str(path))

    @property
    def path(self) -> Path:
        """The dataset directory this reader was constructed with."""
        return self._internal.path

    @property
    def version(self) -> Literal["v2", "v3"]:
        """The detected dataset format version: `"v2"` or `"v3"`."""
        return self._internal.version

    def __repr__(self) -> str:
        return f"LeRobotReader({self._internal.path})"

    # TODO(RR-5278): Gracefully handle the case of a partial dataset.
    # To return only the parsed episodes, we might need to adjust the internal logic.
    def episodes(self) -> list[int]:
        """The episode indices available in this dataset, ascending."""
        return self._internal.episodes()

    def stream(
        self,
        episode: int,
        *,
        entity_path_prefix: str | None = None,
        timeline: str | None = None,
        video_mode: Literal["native", "skip"] = "native",
    ) -> LazyChunkStream:
        """
        Return a lazy stream over one episode's chunks.

        Most video streams directly; a stream that must be re-encoded — H.264 with
        B-frames, or an episode window starting mid-GOP — needs ffmpeg on the system
        `PATH`.

        Each call is independent: the same reader can stream several episodes (or the
        same episode several times) with different configurations. The typical loop is:

        ```python
        reader = LeRobotReader("path/to/dataset")
        for episode in reader.episodes():
            reader.stream(episode).write_rrd(
                f"episode_{episode}.rrd",
                application_id="my_dataset",
                recording_id=f"episode_{episode}",
            )
        ```

        Parameters
        ----------
        episode:
            The episode index to stream; must be one of `episodes()`.
        entity_path_prefix:
            Prepended to every feature's entity path.
        timeline:
            Overrides the derived timeline name (`frame_index` or `timestamp`).
        video_mode:
            `"native"` emits video as-is (v2: whole-file asset, v3: stream samples cut
            to the episode's time window); `"skip"` omits video features.

        Raises
        ------
        ValueError
            If `episode` is not a valid episode index or the configuration is invalid.
            Data problems surface while the stream is drained, not here.

        """
        return LazyChunkStream(
            self._internal.stream(
                episode,
                entity_path_prefix=entity_path_prefix,
                timeline=timeline,
                video_mode=video_mode,
            ),
        )
