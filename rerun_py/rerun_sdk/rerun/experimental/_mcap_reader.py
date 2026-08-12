from __future__ import annotations

from dataclasses import dataclass
from types import MappingProxyType
from typing import TYPE_CHECKING, Literal

from rerun_bindings import McapReaderInternal

from ._lazy_chunk_stream import LazyChunkStream

if TYPE_CHECKING:
    from collections.abc import Mapping, Sequence
    from pathlib import Path

    from rerun_bindings.rerun_bindings import _McapInfoInternal


@dataclass(frozen=True)
class McapSchemaInfo:
    """Schema metadata referenced by an MCAP channel, excluding the schema payload."""

    id: int
    name: str
    encoding: str
    data_size_bytes: int


@dataclass(frozen=True)
class McapChannelInfo:
    """Information about one MCAP channel."""

    id: int
    topic: str
    message_encoding: str
    metadata: Mapping[str, str]
    schema: McapSchemaInfo | None
    message_count: int | None
    frequency_hz: tuple[float, float] | None


@dataclass(frozen=True)
class McapCompressionInfo:
    """Aggregate information for one MCAP chunk compression codec."""

    codec: str
    chunk_count: int
    compressed_size_bytes: int
    uncompressed_size_bytes: int

    @property
    def savings_ratio(self) -> float | None:
        """Return the fraction of uncompressed bytes removed by compression."""
        if self.uncompressed_size_bytes == 0:
            return None
        return 1.0 - self.compressed_size_bytes / self.uncompressed_size_bytes


@dataclass(frozen=True)
class McapChunkInfo:
    """Aggregate information about indexed MCAP chunks."""

    count: int
    max_uncompressed_size_bytes: int | None
    max_compressed_size_bytes: int | None
    has_overlapping_time_ranges: bool


@dataclass(frozen=True)
class McapInfo:
    """
    Header and summary information about a complete MCAP file.

    This joins channels with their schemas, aggregates chunk compression, and provides derived time
    and frequency values. It contains no message or schema payloads and is unaffected by the reader's
    decoder, topic, and time filters.
    """

    profile: str
    library: str
    message_count: int | None
    message_start_time_ns: int | None
    message_end_time_ns: int | None
    duration_ns: int | None
    schema_count: int
    channel_count: int
    attachment_count: int
    metadata_count: int
    statistics_present: bool
    summary_source: Literal["embedded", "reconstructed"]
    chunks: McapChunkInfo
    compression: tuple[McapCompressionInfo, ...]
    channels: tuple[McapChannelInfo, ...]


def _mcap_info_from_internal(info: _McapInfoInternal) -> McapInfo:
    compression = tuple(
        McapCompressionInfo(
            codec=item.codec,
            chunk_count=item.chunk_count,
            compressed_size_bytes=item.compressed_size_bytes,
            uncompressed_size_bytes=item.uncompressed_size_bytes,
        )
        for item in info.compression
    )

    channels = []
    for channel in info.channels:
        raw_schema = channel.schema
        schema = (
            None
            if raw_schema is None
            else McapSchemaInfo(
                id=raw_schema.id,
                name=raw_schema.name,
                encoding=raw_schema.encoding,
                data_size_bytes=raw_schema.data_size_bytes,
            )
        )
        channels.append(
            McapChannelInfo(
                id=channel.id,
                topic=channel.topic,
                message_encoding=channel.message_encoding,
                metadata=MappingProxyType(dict(channel.metadata)),
                schema=schema,
                message_count=channel.message_count,
                frequency_hz=channel.frequency_hz,
            )
        )

    chunks = info.chunks
    return McapInfo(
        profile=info.profile,
        library=info.library,
        message_count=info.message_count,
        message_start_time_ns=info.message_start_time_ns,
        message_end_time_ns=info.message_end_time_ns,
        duration_ns=info.duration_ns,
        schema_count=info.schema_count,
        channel_count=info.channel_count,
        attachment_count=info.attachment_count,
        metadata_count=info.metadata_count,
        statistics_present=info.statistics_present,
        summary_source=info.summary_source,
        chunks=McapChunkInfo(
            count=chunks.count,
            max_uncompressed_size_bytes=chunks.max_uncompressed_size_bytes,
            max_compressed_size_bytes=chunks.max_compressed_size_bytes,
            has_overlapping_time_ranges=chunks.has_overlapping_time_ranges,
        ),
        compression=compression,
        channels=tuple(channels),
    )


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
        recover: bool = False,
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
        recover:
            Whether to recover a missing or invalid MCAP summary in memory. Our reader normally
            requires the summary + chunk index that live at the end of the file, so an interrupted
            recording (valid start, truncated tail, no footer/summary) fails to read. When `recover`
            is set, the summary is reconstructed from a front-to-back scan instead: the incomplete
            tail chunk/record is dropped with a warning, and any channel declared only in the
            dropped tail is lost. The recovered statistics only count the channels and messages that
            could be recovered. Healthy files are unaffected.

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
            recover=recover,
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

    def info(self) -> McapInfo:
        """
        Return structured information about the complete MCAP file.

        The underlying file information is assembled and cached in Rust on first use. Decoder
        selection, topic filters, time filters, timeline type, and timestamp offset do not affect it.
        With `recover=True`, a reconstructed summary describes only the recoverable portion of a
        damaged or incomplete file.
        """
        return _mcap_info_from_internal(self._internal.info())

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
