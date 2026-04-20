from __future__ import annotations

from typing import TYPE_CHECKING, Literal

from rerun.catalog import ContentFilter
from rerun_bindings import LazyChunkStreamInternal

from ._chunk import Chunk
from ._lens import Lens

if TYPE_CHECKING:
    from collections.abc import Callable, Iterable, Iterator, Sequence
    from pathlib import Path

    from rerun_bindings import ChunkInternal, ComponentDescriptor

    from ._chunk_store import ChunkStore
    from ._optimization_settings import OptimizationSettings


class LazyChunkStream:
    """
    A lazy, composable pipeline over chunks.

    Builder methods (``filter``, ``drop``, ``split``, ``merge``) **consume** the input stream(s)
    and return new stream(s). A consumed stream cannot be used as a builder input again; attempting
    to do so raises a ``ValueError``. This prevents accidental reuse that would result in duplicate
    use of the same stream in a pipeline.

    Terminal methods (``collect``, ``write_rrd``, ``__iter__``) do **not** consume the stream and
    may be called repeatedly. Each call creates a fresh execution of the pipeline.
    """

    _internal: LazyChunkStreamInternal

    def __init__(self, internal: LazyChunkStreamInternal) -> None:
        self._internal = internal

    # --- Structured filtering ---

    def filter(
        self,
        *,
        content: ContentFilter | str | Sequence[str] | None = None,
        has_timeline: str | None = None,
        is_static: bool | None = None,
        components: ComponentDescriptor | str | Sequence[ComponentDescriptor | str] | None = None,
    ) -> LazyChunkStream:
        """
        Keep the matching portion of each chunk; drop the rest. Consumes this stream.

        All criteria are combined with AND. For chunk-level predicates (``content``,
        ``has_timeline``, ``is_static``) the chunk either passes or is dropped
        entirely. For ``components``, the chunk is split by component columns:
        only matching component columns are kept (timelines and entity
        path are preserved). When a list is given, any column matching
        any of the listed components is kept (OR semantics). Chunks that
        contain none of the listed components are dropped entirely.

        If a chunk fails any predicate, it is dropped entirely -- no
        component splitting occurs.

        Parameters
        ----------
        content:
            Entity path filter. Accepts a single expression, a list of expressions,
            or a ``ContentFilter`` object.
        has_timeline:
            Only keep chunks that have a column for this timeline.
        is_static:
            If ``True``, keep only static chunks. If ``False``, keep only temporal chunks.
        components:
            Keep only the listed component columns. Accepts ``ComponentDescriptor`` objects
            or ``str`` component identifiers (e.g. ``"Points3D:positions"``).
            A single value or a list are both accepted.

        """
        return LazyChunkStream(
            self._internal.filter(
                content=_normalize_content(content),
                has_timeline=has_timeline,
                is_static=is_static,
                components=_normalize_components(components),
            )
        )

    def drop(
        self,
        *,
        content: ContentFilter | str | Sequence[str] | None = None,
        has_timeline: str | None = None,
        is_static: bool | None = None,
        components: ComponentDescriptor | str | Sequence[ComponentDescriptor | str] | None = None,
    ) -> LazyChunkStream:
        """
        Drop the matching portion of each chunk; keep the rest. Consumes this stream.

        Complement of ``filter()``: what ``filter()`` would keep is
        discarded, what it would discard is kept.

        Parameters
        ----------
        content:
            Entity path filter. Accepts a single expression, a list of expressions,
            or a ``ContentFilter`` object.
        has_timeline:
            Only drop chunks that have a column for this timeline.
        is_static:
            If ``True``, drop only static chunks. If ``False``, drop only temporal chunks.
        components:
            Drop the listed component columns. Accepts ``ComponentDescriptor`` objects
            or ``str`` component identifiers (e.g. ``"Points3D:positions"``).
            A single value or a list are both accepted.

        """
        return LazyChunkStream(
            self._internal.drop_matching(
                content=_normalize_content(content),
                has_timeline=has_timeline,
                is_static=is_static,
                components=_normalize_components(components),
            )
        )

    # --- Map / FlatMap ---

    def map(self, fn: Callable[[Chunk], Chunk]) -> LazyChunkStream:
        """
        Apply a Python function to each chunk, producing exactly one output chunk.

        Runs in Python (GIL-bound, sequential). For transforms that may produce
        zero or many chunks, use ``flat_map`` instead.
        """

        def _wrapper(internal: ChunkInternal) -> ChunkInternal:
            return fn(Chunk(internal))._internal

        return LazyChunkStream(self._internal.map(_wrapper))

    def flat_map(self, fn: Callable[[Chunk], Iterable[Chunk]]) -> LazyChunkStream:
        """
        Apply a Python function to each chunk, producing zero or more output chunks.

        Runs in Python (GIL-bound, sequential).
        """

        def _wrapper(internal: ChunkInternal) -> list[ChunkInternal]:
            return [c._internal for c in fn(Chunk(internal))]

        return LazyChunkStream(self._internal.flat_map(_wrapper))

    # --- Lenses ---

    def lenses(
        self,
        lenses: Sequence[Lens] | Lens,
        *,
        output_mode: Literal["drop_unmatched", "forward_unmatched", "forward_all"] = "drop_unmatched",
    ) -> LazyChunkStream:
        """
        Apply lenses to transform chunk data. Consumes this stream.

        Each lens matches chunks by entity path and input component,
        then transforms the data according to its output specifications.

        Parameters
        ----------
        lenses:
            One or more [`Lens`][] objects describing the transformations.
        output_mode:
            How to handle unmatched chunks:

            - ``"forward_all"``: forward both transformed and original data
            - ``"forward_unmatched"``: forward transformed if matched, otherwise original
            - ``"drop_unmatched"``: only forward transformed data (default)

        """
        if isinstance(lenses, Lens):
            lenses = [lenses]
        return LazyChunkStream(
            self._internal.lenses(
                [lens._internal for lens in lenses],
                output_mode,
            )
        )

    # --- Routing ---

    def split(
        self,
        *,
        content: ContentFilter | str | Sequence[str] | None = None,
        has_timeline: str | None = None,
        is_static: bool | None = None,
        components: ComponentDescriptor | str | Sequence[ComponentDescriptor | str] | None = None,
    ) -> tuple[LazyChunkStream, LazyChunkStream]:
        """
        Split into (matching, non_matching). Consumes this stream.

        Equivalent to ``(stream.filter(\u2026), stream.drop(\u2026))``, but the
        upstream executes only once. ``merge(matching, non_matching)``
        reconstructs the original stream in a semantically lossless way
        (component-wise chunk splitting is not undone).

        Both branches share the same upstream -- it executes once.
        Both branches MUST be consumed for the pipeline to complete
        (dropping an unconsumed branch is fine and unblocks the other).

        Parameters
        ----------
        content:
            Entity path filter. Accepts a single expression, a list of expressions,
            or a ``ContentFilter`` object.
        has_timeline:
            Only match chunks that have a column for this timeline.
        is_static:
            If ``True``, match only static chunks. If ``False``, match only temporal chunks.
        components:
            Match the listed component columns. Accepts ``ComponentDescriptor`` objects
            or ``str`` component identifiers (e.g. ``"Points3D:positions"``).
            A single value or a list are both accepted.

        """
        a, b = self._internal.split(
            content=_normalize_content(content),
            has_timeline=has_timeline,
            is_static=is_static,
            components=_normalize_components(components),
        )
        return LazyChunkStream(a), LazyChunkStream(b)

    @staticmethod
    def merge(*streams: LazyChunkStream) -> LazyChunkStream:
        """
        Merge chunks from multiple streams into one. Consumes all input streams.

        All inputs execute concurrently. Chunks are yielded as they
        become available. Within each input, chunk order is preserved.
        Across inputs, ordering is non-deterministic.
        """
        internals = [s._internal for s in streams]
        return LazyChunkStream(LazyChunkStreamInternal.merge(internals))

    # --- Terminals (trigger execution) ---

    def write_rrd(
        self,
        path: str | Path,
        *,
        application_id: str,
        recording_id: str,
    ) -> None:
        """
        Consume the stream and write all chunks to an RRD file.

        The caller must provide application_id and recording_id explicitly.
        """
        self._internal.write_rrd(
            str(path),
            application_id,
            recording_id,
        )

    def collect(
        self,
        *,
        optimize: OptimizationSettings | None = None,
    ) -> ChunkStore:
        """
        Consume the stream and materialize all chunks into a ChunkStore.

        By default, only the single-pass compaction that happens naturally
        during chunk insertion is applied. Pass ``optimize=OptimizationSettings(...)``
        to run additional optimization (extra convergence passes, video GoP
        rebatching); the defaults for [`OptimizationSettings`][rerun.experimental.OptimizationSettings]
        mirror those of the ``rerun rrd compact`` CLI.

        Parameters
        ----------
        optimize:
            If ``None`` (default), no extra optimization is performed beyond
            the single pass that happens on insert.

            Otherwise, apply the given settings after insertion.

        Examples
        --------
        Run optimization with default settings (matches ``rerun rrd compact``):

        ```python
        store = reader.stream().collect(optimize=OptimizationSettings())
        ```

        """
        from ._chunk_store import ChunkStore

        if optimize is None:
            return ChunkStore(self._internal.collect())
        return ChunkStore(
            self._internal.collect(
                max_bytes=optimize.max_bytes,
                max_rows=optimize.max_rows,
                max_rows_if_unsorted=optimize.max_rows_if_unsorted,
                extra_passes=optimize.extra_passes,
                gop_batching=optimize.gop_batching,
            ),
        )

    def to_chunks(self) -> list[Chunk]:
        """Consume the stream and return all chunks as a list."""
        return [Chunk(internal) for internal in self._internal.to_chunks()]

    def __iter__(self) -> Iterator[Chunk]:
        """Iterate over chunks one at a time (triggers execution)."""
        for internal in self._internal:
            yield Chunk(internal)

    # --- Interop ---

    @staticmethod
    def from_iter(chunks: Iterable[Chunk]) -> LazyChunkStream:
        """
        Wrap a Python iterable of Chunks into a LazyChunkStream.

        Enables user-defined sources and the generator escape hatch.
        """
        # We need to pass an iterable of ChunkInternal objects
        internal_iter = (c._internal for c in chunks)
        return LazyChunkStream(LazyChunkStreamInternal.from_iter(internal_iter))

    def __repr__(self) -> str:
        # TODO(ab): improve this
        return "LazyChunkStream()"


def _normalize_content(
    content: ContentFilter | str | Sequence[str] | None,
) -> list[str] | None:
    """Normalize content to a list of entity path filter expressions for the Rust layer."""
    if content is None:
        return None
    if isinstance(content, str):
        return [content]
    if isinstance(content, ContentFilter):
        return content.to_exprs()
    return list(content)


def _normalize_components(
    components: ComponentDescriptor | str | Sequence[ComponentDescriptor | str] | None,
) -> list[str] | None:
    """Normalize components to a list of ComponentIdentifier strings for the Rust layer."""
    if components is None:
        return None
    if isinstance(components, str):
        return [components]

    # Single ComponentDescriptor
    from rerun_bindings import ComponentDescriptor as CD

    if isinstance(components, CD):
        return [components.component]

    # Sequence of str | ComponentDescriptor
    return [c if isinstance(c, str) else c.component for c in components]
