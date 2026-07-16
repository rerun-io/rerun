from __future__ import annotations

from typing import TYPE_CHECKING, Literal

from rerun_bindings import McapReaderInternal

from ._lazy_chunk_stream import LazyChunkStream

if TYPE_CHECKING:
    from collections.abc import Sequence
    from pathlib import Path


class McapReader:
    """Read chunks from an MCAP file."""

    _internal: McapReaderInternal

    # TODO(ab): this API is a reflection of the current state of the MCAP reader and mirrors `rerun mcap convert`. It's
    #  far from perfect and should be improved as the MCAP reader stabilizes.
    def __init__(
        self,
        path: str | Path,
        *,
        timeline_type: Literal["timestamp", "duration"] = "timestamp",
        timestamp_offset_ns: int | None = None,
        decoders: Sequence[str] | None = None,
        include_topic_regex: Sequence[str] | None = None,
        exclude_topic_regex: Sequence[str] | None = None,
        start_time_ns: int | None = None,
        end_time_ns: int | None = None,
    ) -> None:
        """
        Construct a new MCAP reader.

        Parameters
        ----------
        path:
            Path to the `.mcap` file to read.
        timeline_type:
            Whether to interpret the MCAP `log_time` column as wall-clock timestamps
            ("timestamp") or as nanosecond durations ("duration").
        timestamp_offset_ns:
            Optional offset in nanoseconds to add to all `TimestampNs` time columns.
        decoders:
            Optional list of MCAP decoder identifiers to enable. If omitted, all
            available decoders are enabled. Use
            [`McapReader.available_decoders`][rerun.experimental.McapReader.available_decoders]
            to enumerate them.
        include_topic_regex:
            Optional list of regex patterns. If provided, only topics matching at
            least one pattern are decoded. Patterns use RE2 syntax and are not
            implicitly anchored.
        exclude_topic_regex:
            Optional list of regex patterns. Topics matching any pattern are
            skipped. Applied after includes. Same syntax as `include_topic_regex`.
        start_time_ns:
            Optional inclusive lower bound on the raw MCAP `log_time` (nanoseconds).
            Messages before this time are skipped. `None` leaves the range open at the start.
        end_time_ns:
            Optional exclusive upper bound on the raw MCAP `log_time` (nanoseconds).
            Messages at or after this time are skipped. `None` leaves the range open
            at the end.

        """
        self._internal = McapReaderInternal(
            str(path),
            timeline_type=timeline_type,
            timestamp_offset_ns=timestamp_offset_ns,
            decoders=list(decoders) if decoders is not None else None,
            include_topic_regex=list(include_topic_regex) if include_topic_regex is not None else None,
            exclude_topic_regex=list(exclude_topic_regex) if exclude_topic_regex is not None else None,
            start_time_ns=start_time_ns,
            end_time_ns=end_time_ns,
        )

    def stream(
        self,
        *,
        start_time_ns: int | None = None,
        end_time_ns: int | None = None,
    ) -> LazyChunkStream:
        """
        Return a lazy stream over the chunks in the MCAP file.

        `start_time_ns` and `end_time_ns` override the values passed to the constructor, for this
        scan only. If either `start_time_ns` or `end_time_ns` are provided both are reset.
        """
        return LazyChunkStream(
            self._internal.stream(
                start_time_ns=start_time_ns,
                end_time_ns=end_time_ns,
            )
        )

    def time_bounds(self) -> tuple[int, int]:
        """Return the `(min, max)` MCAP `log_time` bounds (nanoseconds, inclusive)."""
        return self._internal.time_bounds()

    @property
    def path(self) -> Path:
        """The file path of the MCAP file."""
        return self._internal.path

    def __repr__(self) -> str:
        return f"McapReader({self._internal.path})"

    @staticmethod
    def available_decoders() -> list[str]:
        """Return the list of all supported MCAP decoder identifiers."""
        return McapReaderInternal.available_decoders()
