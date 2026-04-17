"""Experimental parquet reader with configurable column grouping and column rules."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

from rerun_bindings import ParquetReaderInternal

from ._lazy_chunk_stream import LazyChunkStream

if TYPE_CHECKING:
    from pathlib import Path


@dataclass(frozen=True)
class ColumnRule:
    """
    Rule for combining columns with matching suffixes into a Rerun component.

    Use the factory methods to create rules:

    - `translation3d()` — 3 columns → `Translation3D`
    - `rotation_quat()` — 4 columns → `RotationQuat`
    - `rotation_axis_angle()` — 4 columns → `RotationAxisAngle`
    - `scale3d()` — 3 columns → `Scale3D`
    - `scalars()` — N columns → `Scalars` with named series
    - `transform()` — 3 + 4 columns → `Transform3D` (translation + rotation)
    """

    suffixes: list[str]
    target: str
    names: list[str] | None = None
    field_name_override: str | None = None
    rotation_suffixes: list[str] | None = None

    @classmethod
    def translation3d(cls, suffixes: list[str], *, field_name_override: str | None = None) -> ColumnRule:
        """Create a rule that combines 3 columns into a ``Translation3D`` component."""
        if len(suffixes) != 3:
            raise ValueError("Translation3D requires exactly 3 suffixes")
        return cls(suffixes, "Translation3D", field_name_override=field_name_override)

    @classmethod
    def rotation_quat(cls, suffixes: list[str], *, field_name_override: str | None = None) -> ColumnRule:
        """Create a rule that combines 4 columns into a ``RotationQuat`` component."""
        if len(suffixes) != 4:
            raise ValueError("RotationQuat requires exactly 4 suffixes")
        return cls(suffixes, "RotationQuat", field_name_override=field_name_override)

    @classmethod
    def rotation_axis_angle(cls, suffixes: list[str], *, field_name_override: str | None = None) -> ColumnRule:
        """Create a rule that combines 4 columns into a ``RotationAxisAngle`` component (3 axis + 1 angle)."""
        if len(suffixes) != 4:
            raise ValueError("RotationAxisAngle requires exactly 4 suffixes (3 axis + 1 angle)")
        return cls(suffixes, "RotationAxisAngle", field_name_override=field_name_override)

    @classmethod
    def scale3d(cls, suffixes: list[str], *, field_name_override: str | None = None) -> ColumnRule:
        """Create a rule that combines 3 columns into a ``Scale3D`` component."""
        if len(suffixes) != 3:
            raise ValueError("Scale3D requires exactly 3 suffixes")
        return cls(suffixes, "Scale3D", field_name_override=field_name_override)

    @classmethod
    def scalars(
        cls,
        suffixes: list[str],
        *,
        names: list[str],
        field_name_override: str | None = None,
    ) -> ColumnRule:
        """Create a rule that combines N columns into a ``Scalars`` component with named series."""
        if len(suffixes) != len(names):
            raise ValueError("suffixes and names must have the same length")
        return cls(suffixes, "Scalars", names=names, field_name_override=field_name_override)

    @classmethod
    def transform(
        cls,
        translation_suffixes: list[str],
        rotation_suffixes: list[str],
        *,
        field_name_override: str | None = None,
    ) -> ColumnRule:
        """
        Create a rule that combines 3 translation + 4 rotation columns into a ``Transform3D``.

        Both suffix sets must match with the same sub-prefix for columns to be
        combined. In struct mode, produces a nested struct with ``translation``
        and ``quaternion`` fields. In flat mode, emits both components at the
        same entity path.
        """
        if len(translation_suffixes) != 3:
            raise ValueError("Transform requires exactly 3 translation suffixes")
        if len(rotation_suffixes) != 4:
            raise ValueError("Transform requires exactly 4 rotation suffixes")
        return cls(
            translation_suffixes,
            "Transform",
            field_name_override=field_name_override,
            rotation_suffixes=rotation_suffixes,
        )


class ParquetReader:
    """Read chunks from a Parquet file."""

    _internal: ParquetReaderInternal

    def __init__(
        self,
        path: str | Path,
        *,
        entity_path_prefix: str | None = None,
        column_grouping: str = "prefix",
        delimiter: str = "_",
        prefixes: list[str] | None = None,
        use_structs: bool = True,
        static_columns: list[str] | None = None,
        index_columns: list[tuple[str, str] | tuple[str, str, str]] | None = None,
        column_rules: list[ColumnRule] | None = None,
    ) -> None:
        """
        Load a parquet file with configurable column grouping and column rules.

        Parameters
        ----------
        path:
            Path to the ``.parquet`` file.
        entity_path_prefix:
            Optional prefix for all entity paths (e.g. ``"/world"``).
        column_grouping:
            How to group columns into chunks. ``"prefix"`` splits column names
            on `delimiter` and groups by the first segment. ``"individual"``
            gives each column its own chunk. ``"explicit_prefixes"`` groups
            columns by the explicit prefix strings in `prefixes`.
        delimiter:
            Character used to split column names when ``column_grouping="prefix"``.
        prefixes:
            Explicit prefix strings for grouping columns. Required when
            ``column_grouping="explicit_prefixes"``. Columns starting with a
            prefix are grouped together; the prefix is stripped from the
            component name. Prefixes are tried longest-first to avoid
            ambiguity.
        use_structs:
            When ``True`` (default) and ``column_grouping="prefix"`` or
            ``"explicit_prefixes"``, columns sharing a prefix are packed into
            a single Arrow ``Struct`` component. When ``False``, each column
            becomes a separate component (the pre-struct layout). Ignored
            when ``column_grouping="individual"``.
        static_columns:
            Column names whose values are constant across all rows. These are
            emitted once as timeless/static data. An error is raised if a
            listed column contains varying values.
        index_columns:
            List of columns to use as timeline indices. Each entry is a tuple:
            ``(name, type)`` or ``(name, type, unit)``.

            The ``type`` specifies the timeline kind:

            - ``"timestamp"``: time since epoch
            - ``"duration"``: elapsed time
            - ``"sequence"``: ordinal integer index

            The ``unit`` describes what the raw integer values in the column
            represent (not a desired output unit). Rerun stores all timestamps
            in nanoseconds internally, so values are scaled accordingly.
            Supported: ``"ns"`` (default), ``"us"``, ``"ms"``, ``"s"``.
            Ignored for ``"sequence"`` type.

            When omitted, a synthetic ``row_index`` sequence timeline is
            generated automatically (one entry per row).
        column_rules:
            Rules for combining columns with matching suffixes into typed
            Rerun components. Each rule is a `ColumnRule` created via
            factory methods. Rules are processed in list order; the first rule
            whose suffixes match wins. Put specific rules before broad
            catch-all rules.

            Example::

                column_rules=[
                    ColumnRule.translation3d(["_pos_x", "_pos_y", "_pos_z"], field_name_override="_pos"),
                    ColumnRule.rotation_quat(["_quat_x", "_quat_y", "_quat_z", "_quat_w"], field_name_override="_quat"),
                    ColumnRule.scalars(["_x", "_y", "_z"], names=["x", "y", "z"]),
                ]

        """
        # Normalize index_columns: pad 2-tuples to 3-tuples with None for the unit
        normalized_index = (
            [(t[0], t[1], t[2] if len(t) > 2 else None) for t in index_columns] if index_columns is not None else None
        )

        self._internal = ParquetReaderInternal(
            str(path),
            entity_path_prefix=entity_path_prefix,
            column_grouping=column_grouping,
            delimiter=delimiter,
            prefixes=prefixes,
            use_structs=use_structs,
            static_columns=static_columns,
            index_columns=normalized_index,
            column_rules=column_rules,
        )

    def stream(self) -> LazyChunkStream:
        """Return a lazy stream over all chunks in the Parquet file."""
        return LazyChunkStream(self._internal.stream())

    @property
    def path(self) -> Path:
        """The file path of the Parquet file."""
        return self._internal.path

    def __repr__(self) -> str:
        return f"ParquetReader({self._internal.path})"
