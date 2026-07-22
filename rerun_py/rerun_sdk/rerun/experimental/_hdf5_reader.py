"""Experimental HDF5 reader mapping groups to entities and datasets to components."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

from rerun_bindings import Hdf5ReaderInternal

from ._lazy_chunk_stream import LazyChunkStream

if TYPE_CHECKING:
    from pathlib import Path

    from ._index_column import IndexColumn


@dataclass(frozen=True)
class DatasetInfo:
    """
    Structural metadata for a single HDF5 dataset.

    Attributes
    ----------
    path:
        Full path of the dataset within the file (e.g. `/observations/qpos`).
    shape:
        Dataset dimensions (e.g. `(272, 128, 128, 3)`).
    dtype:
        Element type name (e.g. `"uint8"`, `"float64"`).

    """

    path: str
    shape: tuple[int, ...]
    dtype: str


class Hdf5Reader:
    """
    Read chunks from an HDF5 file.

    The reader is a lightweight handle over the file: inspect the raw structure
    with `groups()`, `datasets()`, and `attributes()`, and produce chunks with
    `stream(...)`. All loading options live on `stream()`, so one reader can drive
    several differently-configured streams over the same file.

    Each HDF5 group is mapped to a Rerun entity, and the group's leaf datasets
    become the columns of that entity. The file root maps to the entity `/`, and
    nested groups map to nested entity paths (`/observations/images` becomes the
    entity `/observations/images`). See `stream()` for how datasets, timelines, and
    attributes are turned into chunks.

    Parameters
    ----------
    path:
        Path to the `.hdf5` / `.h5` file.

    Raises
    ------
    FileNotFoundError
        If `path` does not exist.

    """

    _internal: Hdf5ReaderInternal

    def __init__(self, path: str | Path) -> None:
        self._internal = Hdf5ReaderInternal(str(path))

    def stream(
        self,
        *,
        entity_path_prefix: str | None = None,
        index_column: IndexColumn | None = None,
        ignore_datasets: list[str] | None = None,
        use_structs: bool = True,
    ) -> LazyChunkStream:
        """
        Return a lazy stream over all chunks in the HDF5 file.

        Each call is independent: the same reader can be streamed several times with
        different configurations.

        Datasets are loaded according to their dimensionality:

        - A 0-D (scalar) dataset is loaded as **static** data — a single value with
          no timeline.
        - A 1-D dataset `[N]` becomes a column of `N` scalar rows.
        - A 2-D dataset `[N, K]` becomes a column of `N` rows, each a fixed-size list
          of `K` elements.
        - A 3-D-or-higher dataset `[N, d1, …, dk]` becomes a column of `N` rows, each
          a single blob of the matching type (an Arrow `List<PRIMITIVE_TYPE>`) holding
          the row's raw row-major values. The original per-row shape is not recorded
          in the emitted data; recover it via [`datasets`][rerun.experimental.Hdf5Reader.datasets].

        For 1-D and higher-dimensional datasets the **leading** dimension is always
        the row axis. Element types are mapped to their natural Arrow equivalents
        (signed and unsigned integers, floats, and strings); no semantic
        interpretation is applied.

        HDF5 attributes are emitted as **static** chunks under a dedicated
        `__hdf5_properties` entity, mirroring the source layout: root attributes land
        on `__hdf5_properties`, and attributes on object `/a/b` on
        `__hdf5_properties/a/b`. Each attribute becomes one static component named
        after it, typed with the same mapping as datasets. This keeps the general
        `__properties` entity free for user-defined property layers.

        Row alignment
        -------------
        Every loaded, non-ignored, non-scalar dataset is aligned positionally to the
        file-wide timeline and must therefore share the same number of rows (scalar
        datasets are static and exempt):

        - With an `index_column`, that shared count is the index dataset's length.
        - Without one, the datasets must all agree on a single row count, which
          becomes the length of the generated `row_index` timeline.

        A dataset that violates this raises unless it is listed in `ignore_datasets`;
        nothing is dropped automatically to satisfy alignment.

        Parameters
        ----------
        entity_path_prefix:
            Optional prefix prepended to every entity path (for example `"/world"`).
        index_column:
            Dataset to use as the file-wide timeline index, built with
            [`IndexColumn`][rerun.experimental.IndexColumn], e.g.
            `IndexColumn.timestamp("/time", input_unit="s")` or
            `IndexColumn.sequence("/frame_id")`.

            The referenced dataset must be 1-dimensional. When omitted, a single
            `row_index` sequence timeline (0, 1, …) is generated for the whole file
            and every loaded dataset must align to it (see Row alignment).
        ignore_datasets:
            Datasets or groups to exclude entirely. Each entry is a dataset path or
            a group path (which excludes the whole subtree). Ignored datasets are
            neither loaded nor considered for row alignment.
        use_structs:
            When `True` (default), all columns of an entity are packed into a single
            Arrow `Struct` component, with one field per dataset named after that
            dataset. When `False`, each dataset becomes a separate component on the
            same entity. A group holding a single dataset always emits that dataset
            as a bare component, never as a one-field struct.

        Raises
        ------
        ValueError
            If a loaded, non-ignored, non-scalar dataset cannot be aligned to the
            applicable row count (the index length, or the file's shared row count
            when no `index_column` is set). Resolve by adding the offending dataset
            to `ignore_datasets` or by choosing a compatible `index_column`.

            Also raised when the file exists but cannot be parsed as HDF5: the
            layout is validated eagerly here, so such failures surface at
            `stream()` rather than lazily mid-iteration.

        """
        return LazyChunkStream(
            self._internal.stream(
                entity_path_prefix=entity_path_prefix,
                index_column=index_column._as_internal_tuple() if index_column is not None else None,
                ignore_datasets=ignore_datasets,
                use_structs=use_structs,
            )
        )

    def groups(self, path: str = "/") -> list[str]:
        """
        List the group paths under `path`, recursively.

        Metadata only — no dataset values are read; reflects the raw file.

        Parameters
        ----------
        path:
            Group under which to list. Defaults to the root group `/`, i.e. the
            whole file.

        """
        return self._internal.groups(path)

    def datasets(self, path: str = "/") -> list[DatasetInfo]:
        """
        List the datasets under `path`, recursively, with their shape and dtype.

        Metadata only — no dataset values are read; reflects the raw file.

        Parameters
        ----------
        path:
            Group under which to list. Defaults to the root group `/`, i.e. the
            whole file.

        """
        return [
            DatasetInfo(path=dataset_path, shape=tuple(shape), dtype=dtype)
            for (dataset_path, shape, dtype) in self._internal.datasets(path)
        ]

    def attributes(self, path: str = "/") -> dict[str, int | float | str | bytes | list[int | float | str]]:
        """
        Read the HDF5 attributes attached to an object as a typed Python dict.

        This is a convenience accessor for the same attributes that `stream()`
        emits under `__hdf5_properties` (see the class docstring). It reads the
        raw file directly.

        Parameters
        ----------
        path:
            Path to the object whose attributes are read. Defaults to the root
            group `/`, i.e. the file-level (global) attributes. May reference
            any group or dataset.

        Returns
        -------
        A mapping from attribute name to value. Scalar attributes are returned
        as Python scalars (`int`, `float`, `str`, `bytes`); array-valued
        attributes are returned as lists. Empty if the object has no attributes.

        Raises
        ------
        KeyError
            If `path` does not exist in the file.

        """
        return self._internal.attributes(path)

    @property
    def path(self) -> Path:
        """The file path of the HDF5 file."""
        return self._internal.path

    def __repr__(self) -> str:
        return f"Hdf5Reader({self._internal.path})"
