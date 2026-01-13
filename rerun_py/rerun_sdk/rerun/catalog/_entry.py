from __future__ import annotations

from abc import ABC
from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, Generic, TypeAlias, TypeVar

import pyarrow as pa
from pyarrow import RecordBatchReader
from typing_extensions import deprecated

from rerun_bindings import (
    DatasetEntryInternal,
    DatasetViewInternal,
    TableEntryInternal,
    TableInsertModeInternal,
)

from . import EntryId

#: Type alias for supported batch input types for TableEntry write methods.
_BatchesType: TypeAlias = (
    RecordBatchReader | pa.RecordBatch | Sequence[pa.RecordBatch] | Sequence[Sequence[pa.RecordBatch]]
)

if TYPE_CHECKING:
    from datetime import datetime
    from typing import Type, Union

    import datafusion

    from rerun import Archetype
    from rerun.archetypes._baseclasses import ComponentMixin
    from rerun.recording import Recording

    from . import (
        CatalogClient,
        ComponentColumnDescriptor,
        ComponentColumnSelector,
        EntryKind,
        IndexColumnSelector,
        IndexConfig,
        IndexingResult,
        IndexValuesLike,
        RegistrationHandle,
        Schema,
        VectorDistanceMetric,
    )

    # Type aliases for archetype and component specs
    ArchetypeSpec = Union[str, Type[Archetype]]
    ComponentTypeSpec = Union[str, Type[ComponentMixin]]


InternalEntryT = TypeVar("InternalEntryT", DatasetEntryInternal, TableEntryInternal)


def _archetype_to_name(archetype: ArchetypeSpec) -> str:  # type: ignore[name-defined]
    """
    Convert archetype spec to fully-qualified name.

    Args:
        archetype: Either a fully-qualified string (e.g., "rerun.archetypes.Points3D")
                   or an Archetype type (e.g., rr.Points3D)

    Returns:
        Fully-qualified archetype name as a string

    Raises:
        ValueError: If string is not fully-qualified (doesn't contain '.')
        AttributeError: If archetype type doesn't have archetype() method
    """
    if isinstance(archetype, str):
        if "." not in archetype:
            raise ValueError(
                f"Archetype name must be fully-qualified (e.g., 'rerun.archetypes.Points3D'), "
                f"got: {archetype!r}. Short names like 'Points3D' are not supported."
            )
        return archetype
    else:
        # It's a Type[Archetype] - get the name via archetype() method
        if not hasattr(archetype, "archetype") or not callable(archetype.archetype):
            raise AttributeError(
                f"Archetype type {archetype!r} must have a callable archetype() method. "
                f"Make sure you're passing a valid Rerun archetype type."
            )
        return archetype.archetype()


def _component_type_to_name(component_type: ComponentTypeSpec) -> str:  # type: ignore[name-defined]
    """
    Convert component type spec to fully-qualified name.

    Args:
        component_type: Either a fully-qualified string (e.g., "rerun.components.Position3D")
                        or a ComponentMixin type (e.g., rr.components.Position3D)

    Returns:
        Fully-qualified component type name as a string

    Raises:
        ValueError: If string is not fully-qualified (doesn't contain '.')
        AttributeError: If component type doesn't have _BATCH_TYPE._COMPONENT_TYPE
    """
    if isinstance(component_type, str):
        if "." not in component_type:
            raise ValueError(
                f"Component type name must be fully-qualified "
                f"(e.g., 'rerun.components.Position3D'), got: {component_type!r}. "
                f"Short names like 'Position3D' are not supported."
            )
        return component_type
    else:
        # It's a Type[ComponentMixin] - get via _BATCH_TYPE._COMPONENT_TYPE
        if not hasattr(component_type, "_BATCH_TYPE"):
            raise AttributeError(
                f"Component type {component_type!r} must have a _BATCH_TYPE attribute. "
                f"Make sure you're passing a valid Rerun component type."
            )
        if not hasattr(component_type._BATCH_TYPE, "_COMPONENT_TYPE"):
            raise AttributeError(
                f"Component type {component_type!r}._BATCH_TYPE must have a _COMPONENT_TYPE attribute. "
                f"Make sure you're passing a valid Rerun component type."
            )
        return component_type._BATCH_TYPE._COMPONENT_TYPE


def _get_column_names_for_archetypes(
    schema: Schema,  # type: ignore[name-defined]
    archetypes: Sequence[ArchetypeSpec],  # type: ignore[name-defined]
) -> list[str]:
    """
    Helper to get column names for a list of archetypes.

    Args:
        schema: The schema to query
        archetypes: List of archetype specs

    Returns:
        List of column names matching the archetypes

    Examples:
        >>> schema = dataset.schema()
        >>> # Get columns for single archetype
        >>> columns = _get_column_names_for_archetypes(schema, [rr.Points3D])
        >>> # Get columns for multiple archetypes
        >>> columns = _get_column_names_for_archetypes(
        ...     schema,
        ...     ["rerun.archetypes.Points3D", "rerun.archetypes.Transform3D"]
        ... )
    """
    all_column_names = []
    for archetype in archetypes:
        archetype_name = _archetype_to_name(archetype)  # type: ignore[arg-type]
        column_names = schema.column_names_for(archetype=archetype_name)
        all_column_names.extend(column_names)
    return all_column_names


def _normalize_to_sequence(value: Any) -> Sequence[Any]:
    """
    Normalize a single value or sequence to a sequence.

    Strings are treated as single values, not sequences, even though
    they are technically iterable.

    Args:
        value: Single value or sequence

    Returns:
        Sequence containing the value(s)

    Examples:
        >>> _normalize_to_sequence(rr.Points3D)
        [<class 'rerun.archetypes.Points3D'>]
        >>> _normalize_to_sequence("rerun.archetypes.Points3D")
        ["rerun.archetypes.Points3D"]
        >>> _normalize_to_sequence([rr.Points3D, rr.Transform3D])
        [<class 'rerun.archetypes.Points3D'>, <class 'rerun.archetypes.Transform3D'>]
    """
    if not isinstance(value, Sequence) or isinstance(value, str):
        return [value]
    return value


def _get_column_names_for_component_types(
    schema: Schema,  # type: ignore[name-defined]
    component_types: Sequence[ComponentTypeSpec],  # type: ignore[name-defined]
) -> list[str]:
    """
    Helper to get column names for a list of component types.

    Args:
        schema: The schema to query
        component_types: List of component type specs

    Returns:
        List of column names matching the component types

    Examples:
        >>> from rerun import components
        >>> schema = dataset.schema()
        >>> # Get columns for single component type
        >>> columns = _get_column_names_for_component_types(schema, [components.Position3D])
        >>> # Get columns for multiple component types
        >>> columns = _get_column_names_for_component_types(
        ...     schema,
        ...     ["rerun.components.Position3D", "rerun.components.Color"]
        ... )
    """
    all_column_names = []
    for component_type in component_types:
        component_type_name = _component_type_to_name(component_type)  # type: ignore[arg-type]
        column_names = schema.column_names_for(component_type=component_type_name)
        all_column_names.extend(column_names)
    return all_column_names


class Entry(ABC, Generic[InternalEntryT]):
    """An entry in the catalog."""

    __slots__ = ("_internal",)

    def __init__(self, inner: InternalEntryT) -> None:
        self._internal: InternalEntryT = inner

    def __repr__(self) -> str:
        return f"Entry({self.kind}, '{self.name}')"

    @property
    def id(self) -> EntryId:
        """The entry's id."""
        return self._internal.entry_details().id

    @property
    def name(self) -> str:
        """The entry's name."""
        return self._internal.entry_details().name

    @property
    def kind(self) -> EntryKind:
        """The entry's kind."""

        return self._internal.entry_details().kind

    @property
    def created_at(self) -> datetime:
        """The entry's creation date and time."""

        return self._internal.entry_details().created_at

    @property
    def updated_at(self) -> datetime:
        """The entry's last updated date and time."""

        return self._internal.entry_details().updated_at

    @property
    def catalog(self) -> CatalogClient:
        """The catalog client that this entry belongs to."""

        from . import CatalogClient

        return CatalogClient._from_internal(self._internal.catalog())

    def delete(self) -> None:
        """Delete this entry from the catalog."""

        self._internal.delete()

    def set_name(self, name: str) -> None:
        """
        Change the name of this entry.

        **Note**: entry names must be unique within the catalog. If the new name is not unique, an error will be raised.

        Parameters
        ----------
        name : str
            New name for the entry

        """
        self._internal.set_name(name)

    @deprecated("Entry.update() is deprecated. Use Entry.set_name() instead.")
    def update(self, *, name: str | None = None) -> None:
        """
        Update this entry's properties.

        .. deprecated::
            Use :meth:`set_name` instead.

        Parameters
        ----------
        name : str | None
            New name for the entry

        """

        if name is not None:
            self._internal.set_name(name)

    def __eq__(self, other: object) -> bool:
        """
        Compare this entry to another object.

        Supports comparison with `str` and `EntryId` to enable the following patterns:
        ```py
        "entry_name" in client.entries()
        entry_id in client.entries()
        ```
        """
        match other:
            case Entry():
                return self.id == other.id

            case str():
                return self.name == other

            case EntryId():
                return self.id == other

            case _:
                return NotImplemented

    # Make it explicit that `Entries` are view objects for which hashing cannot sensibly be implemented
    __hash__ = None  # type: ignore[assignment]


class DatasetEntry(Entry[DatasetEntryInternal]):
    """A dataset entry in the catalog."""

    @property
    def manifest_url(self) -> str:
        """Return the dataset manifest URL."""

        return self._internal.manifest_url

    def arrow_schema(self) -> pa.Schema:
        """Return the Arrow schema of the data contained in the dataset."""

        return self._internal.arrow_schema()

    def register_blueprint(self, uri: str, set_default: bool = True) -> None:
        """
        Register an existing .rbl visible to the server.

        By default, also set this blueprint as default.
        """

        blueprint_dataset = self.blueprint_dataset()

        if blueprint_dataset is None:
            raise LookupError("a blueprint dataset is not configured for this dataset")

        segment_id = blueprint_dataset.register(uri).wait().segment_ids[0]

        if set_default:
            self.set_default_blueprint(segment_id)

    def blueprints(self) -> list[str]:
        """Lists all blueprints currently registered with this dataset."""

        blueprint_dataset = self.blueprint_dataset()
        if blueprint_dataset is None:
            return []
        else:
            return blueprint_dataset.segment_ids()

    def set_default_blueprint(self, blueprint_name: str | None) -> None:
        """Set an already-registered blueprint as default for this dataset."""

        return self._internal.set_default_blueprint_segment_id(blueprint_name)

    def default_blueprint(self) -> str | None:
        """Return the name currently set blueprint."""

        return self._internal.default_blueprint_segment_id()

    def blueprint_dataset(self) -> DatasetEntry | None:
        """The associated blueprint dataset, if any."""

        ds = self._internal.blueprint_dataset()
        return None if ds is None else DatasetEntry(ds)

    def schema(self) -> Schema:
        """Return the schema of the data contained in the dataset."""
        from ._schema import Schema

        return Schema(self._internal.schema())

    def segment_ids(self) -> list[str]:
        """Returns a list of segment IDs for the dataset."""

        return self._internal.segment_ids()

    def segment_table(
        self,
        join_meta: TableEntry | datafusion.DataFrame | None = None,
        join_key: str = "rerun_segment_id",
    ) -> datafusion.DataFrame:
        """
        Return the segment table as a DataFusion DataFrame.

        The segment table contains metadata about each segment in the dataset,
        including segment IDs, layer names, storage URLs, and size information.

        Parameters
        ----------
        join_meta
            Optional metadata table or DataFrame to join with the segment table.
            If a `TableEntry` is provided, it will be converted to a DataFrame
            using `reader()`.
        join_key
            The column name to use for joining, defaults to "rerun_segment_id".
            Both the segment table and `join_meta` must contain this column.

        Returns
        -------
        datafusion.DataFrame
            The segment metadata table, optionally joined with `join_meta`.

        """
        segment_table_df = self._internal.segment_table()

        if join_meta is not None:
            if isinstance(join_meta, TableEntry):
                join_meta = join_meta.reader()

            if join_key not in segment_table_df.schema().names:
                raise ValueError(f"Dataset segment table must contain join_key column '{join_key}'.")
            if join_key not in join_meta.schema().names:
                raise ValueError(f"join_meta must contain join_key column '{join_key}'.")

            meta_join_key = join_key + "_meta"
            join_meta = join_meta.with_column_renamed(join_key, meta_join_key)

            return segment_table_df.join(
                join_meta,
                left_on=join_key,
                right_on=meta_join_key,
                how="left",
            ).drop(meta_join_key)

        return segment_table_df

    def manifest(self) -> datafusion.DataFrame:
        """Return the dataset manifest as a DataFusion DataFrame."""

        return self._internal.manifest()

    def segment_url(  # noqa: PLR0917
        self,
        segment_id: str,
        timeline: str | None = None,
        start: datetime | int | None = None,
        end: datetime | int | None = None,
    ) -> str:
        """
        Return the URL for the given segment.

        Parameters
        ----------
        segment_id: str
            The ID of the segment to get the URL for.

        timeline: str | None
            The name of the timeline to display.

        start: int | datetime | None
            The start selected time for the segment.
            Integer for ticks, or datetime/nanoseconds for timestamps.

        end: int | datetime | None
            The end selected time for the segment.
            Integer for ticks, or datetime/nanoseconds for timestamps.

        Examples
        --------
        # With ticks
        >>> start_tick, end_time = 0, 10
        >>> dataset.segment_url("some_id", "log_tick", start_tick, end_time)

        # With timestamps
        >>> start_time, end_time = datetime.now() - timedelta(seconds=4), datetime.now()
        >>> dataset.segment_url("some_id", "real_time", start_time, end_time)

        Returns
        -------
        str
            The URL for the given segment.

        """

        return self._internal.segment_url(segment_id, timeline, start, end)

    def register(
        self,
        recording_uri: str | Sequence[str],
        *,
        layer_name: str | Sequence[str] = "base",
    ) -> RegistrationHandle:
        """
        Register RRD URIs to the dataset and return a handle to track progress.

        This method initiates the registration of recordings to the dataset, and returns
        a handle that can be used to wait for completion or iterate over results.

        Parameters
        ----------
        recording_uri
            The URI(s) of the RRD(s) to register. Can be a single URI string or a sequence of URIs.

        layer_name
            The layer(s) to which the recordings will be registered to.
            Can be a single layer name (applied to all recordings) or a sequence of layer names
            (must match the length of `recording_uri`).
            Defaults to `"base"`.

        Returns
        -------
        RegistrationHandle
            A handle to track and wait on the registration tasks.

        """
        from ._registration_handle import RegistrationHandle

        if isinstance(recording_uri, str):
            recording_uris = [recording_uri]
        else:
            recording_uris = list(recording_uri)

        if isinstance(layer_name, str):
            layer_names = [layer_name] * len(recording_uris)
        else:
            layer_names = list(layer_name)
            if len(layer_names) != len(recording_uris):
                raise ValueError("`layer_name` must be the same length as `recording_uri`")

        return RegistrationHandle(self._internal.register(recording_uris, recording_layers=layer_names))

    def register_prefix(self, recordings_prefix: str, layer_name: str | None = None) -> RegistrationHandle:
        """
        Register all RRDs under a given prefix to the dataset and return a handle to track progress.

        A prefix is a directory-like path in an object store (e.g. an S3 bucket or ABS container).
        All RRDs that are recursively found under the given prefix will be registered to the dataset.

        This method initiates the registration of the recordings to the dataset, and returns
        a handle that can be used to wait for completion or iterate over results.

        Parameters
        ----------
        recordings_prefix: str
            The prefix under which to register all RRDs.

        layer_name: Optional[str]
            The layer to which the recordings will be registered to.
            If `None`, this defaults to `"base"`.

        Returns
        -------
        A handle to track and wait on the registration tasks.

        """
        from ._registration_handle import RegistrationHandle

        return RegistrationHandle(self._internal.register_prefix(recordings_prefix, layer_name=layer_name))

    def download_segment(self, segment_id: str) -> Recording:
        """Download a segment from the dataset."""

        return self._internal.download_segment(segment_id)

    def filter_segments(self, segment_ids: str | Sequence[str] | datafusion.DataFrame) -> DatasetView:
        """
        Return a new DatasetView filtered to the given segment IDs.

        Parameters
        ----------
        segment_ids
            A segment ID string, a list of segment ID strings, or a DataFusion DataFrame
            with a column named 'rerun_segment_id'. When passing a DataFrame,
            if there are additional columns, they will be ignored.

        Returns
        -------
        DatasetView
            A new view filtered to the given segments.

        Examples
        --------
        ```python
        # Filter to a single segment
        view = dataset.filter_segments("recording_0")

        # Filter to specific segments
        view = dataset.filter_segments(["recording_0", "recording_1"])

        # Filter using a DataFrame
        good_segments = segment_table.filter(col("success"))
        view = dataset.filter_segments(good_segments)

        # Read data from the filtered view
        df = view.reader(index="timeline")
        ```

        """

        import datafusion

        if isinstance(segment_ids, str):
            segment_ids = [segment_ids]
        elif isinstance(segment_ids, datafusion.DataFrame):
            segment_ids = segment_ids.select("rerun_segment_id").to_pydict()["rerun_segment_id"]

        return DatasetView(self._internal.filter_segments(list(segment_ids)))

    def filter_contents(
        self, exprs: str | Sequence[str]
    ) -> DatasetView:
        """
        Return a new DatasetView filtered to the given entity paths or specific component columns.

        This method accepts both entity path patterns and component column selectors:
        - **Entity path patterns** (no colon): Include all components at matching entities
        - **Component column selectors** (with colon): Include only specific components

        The format automatically detects which mode to use based on the presence of `:` in the string.

        Entity path expressions support wildcards:
        - `"/points/**"` matches all entities under /points
        - `"-/text/**"` excludes all entities under /text

        Component column selectors use the format: `"entity_path:component"`
        - Example: `"/world/points:Position3D"`
        - This matches the format used by blueprint APIs (ComponentColumnSelector)

        Parameters
        ----------
        exprs : str | Sequence[str]
            Can be:
            - Single string (entity path or column selector)
            - List of strings (can mix entity paths and column selectors)

            Entity path examples:
            - `"/points/**"` - All components at entities under /points
            - `"/world/camera"` - All components at /world/camera
            - `"-/text/**"` - Exclude entities under /text

            Column selector examples:
            - `"/world/points:Position3D"` - Only Position3D component at /world/points
            - `"/world/points:Points3D:positions"` - Specific archetype component

        Returns
        -------
        DatasetView
            A new view filtered to the matching entity paths/components.

        Notes
        -----
        Edge cases and special behavior:

        - **Property paths**: Entity paths starting with `"property:"` or containing
          `"/__properties:"` are treated as entity paths (not column selectors), even
          though they contain colons.
        - **Mixing modes**: Cannot mix entity path patterns and column selectors in the
          same call. Use either entity paths (wildcards allowed) OR column selectors
          (exact paths only), not both. Raises `ValueError` if mixed.
        - **Empty filter**: Passing an empty list `[]` returns all columns. To get an
          empty view, filter to a non-existent path.
        - **Invalid selectors**: Invalid column selector strings raise `ValueError` with
          a clear error message.

        Examples
        --------
        Entity path filtering (all components):
        ```python
        # Filter to a single entity path
        view = dataset.filter_contents("/points/**")

        # Filter to specific entity paths
        view = dataset.filter_contents(["/points/**", "/camera/**"])

        # Exclude certain paths
        view = dataset.filter_contents(["/points/**", "-/text/**"])
        ```

        Component column filtering (specific components):
        ```python
        # Filter to specific components
        view = dataset.filter_contents([
            "/world/points:Position3D",
            "/world/points:Color"
        ])

        # Mix entity paths and column selectors
        view = dataset.filter_contents([
            "/world/points:Position3D",  # Only Position3D at /world/points
            "/world/camera"              # All components at /world/camera
        ])
        ```

        Chaining with other filters:
        ```python
        view = (dataset
            .filter_segments(["recording_0"])
            .filter_contents(["/world/points:Position3D"]))
        ```

        See Also
        --------
        filter_archetypes : Filter to specific archetype types (higher-level filtering)
        filter_component_types : Filter to specific component types (higher-level filtering)
        schema.column_names_for : Get column names for filtering programmatically

        """

        if isinstance(exprs, str):
            exprs = [exprs]

        # Separate entity paths from column selectors based on presence of ":"
        entity_paths = []
        column_selectors = []

        for expr in exprs:
            # Property paths contain ":" but are entity paths, not column selectors
            # Column selectors have format "entity_path:component"
            # Property paths start with "property:" or contain "/__properties:"
            is_property_path = expr.startswith("property:") or "/__properties:" in expr

            if ":" in expr and not expr.startswith("-") and not is_property_path:
                # It's a column selector (has colon, not a negation, and not a property path)
                column_selectors.append(expr)
            else:
                # It's an entity path pattern (no colon, is a negation, or is a property path)
                entity_paths.append(expr)

        # If we have column selectors, use component-level filtering
        if column_selectors:
            if entity_paths:
                # Mixed mode not supported: raise clear error
                raise ValueError(
                    "Cannot mix entity path patterns and column selectors in the same filter. "
                    "Please use either entity paths (no colon) or column selectors (with colon), not both."
                )
            return DatasetView(self._internal.filter_contents_columns(column_selectors))

        # Otherwise use entity-path-only filtering (current behavior)
        return DatasetView(self._internal.filter_contents(entity_paths))

    def filter_archetypes(
        self,
        archetypes: ArchetypeSpec | Sequence[ArchetypeSpec],  # type: ignore[name-defined]
    ) -> DatasetView:
        """
        Return a new DatasetView filtered to the given archetypes.

        This filters the dataset to only include columns belonging to the specified
        archetype(s). You can pass archetype types directly (e.g., `rr.Points3D`)
        or fully-qualified string names (e.g., `"rerun.archetypes.Points3D"`).

        Parameters
        ----------
        archetypes : str | Type[Archetype] | Sequence[str | Type[Archetype]]
            Archetype or list of archetypes. Can be:
            - A single archetype type (e.g., `rr.Points3D`)
            - A fully-qualified string (e.g., `"rerun.archetypes.Points3D"`)
            - A list of either

        Returns
        -------
        DatasetView
            A new view filtered to the matching archetypes.

        Examples
        --------
        ```python
        import rerun as rr

        # Filter to a single archetype using type
        view = dataset.filter_archetypes(rr.Points3D)

        # Filter using string
        view = dataset.filter_archetypes("rerun.archetypes.Points3D")

        # Filter to multiple archetypes
        view = dataset.filter_archetypes([rr.Points3D, rr.Transform3D])

        # Chain with other filters
        view = dataset.filter_segments(["recording_0"]).filter_archetypes(rr.Points3D)

        # Read filtered data
        df = view.reader(index="frame")
        ```

        """
        archetypes = _normalize_to_sequence(archetypes)

        # Get column names using shared helper
        all_column_names = _get_column_names_for_archetypes(self.schema(), archetypes)

        # Use filter_contents with the column names
        if not all_column_names:
            # No matching columns - return empty view by filtering to non-existent entity path
            # Note: Empty list would return ALL columns, so we use a path that won't match anything
            return self.filter_contents(["/__rerun_internal__/non_existent"])

        return self.filter_contents(all_column_names)

    def filter_component_types(
        self,
        component_types: ComponentTypeSpec | Sequence[ComponentTypeSpec],  # type: ignore[name-defined]
    ) -> DatasetView:
        """
        Return a new DatasetView filtered to the given component types.

        This filters the dataset to only include columns of the specified
        component type(s). You can pass component types directly (e.g.,
        `rr.components.Position3D`) or fully-qualified string names (e.g.,
        `"rerun.components.Position3D"`).

        Parameters
        ----------
        component_types : str | Type[ComponentMixin] | Sequence[str | Type[ComponentMixin]]
            Component type or list of component types. Can be:
            - A single component type (e.g., `rr.components.Position3D`)
            - A fully-qualified string (e.g., `"rerun.components.Position3D"`)
            - A list of either

        Returns
        -------
        DatasetView
            A new view filtered to the matching component types.

        Examples
        --------
        ```python
        import rerun as rr
        from rerun import components

        # Filter to a single component type
        view = dataset.filter_component_types(components.Position3D)

        # Filter using string
        view = dataset.filter_component_types("rerun.components.Position3D")

        # Filter to multiple component types
        view = dataset.filter_component_types([components.Position3D, components.Color])

        # Chain with other filters
        view = dataset.filter_segments(["recording_0"]).filter_component_types(components.Position3D)

        # Read filtered data
        df = view.reader(index="frame")
        ```

        """
        component_types = _normalize_to_sequence(component_types)

        # Get column names using shared helper
        all_column_names = _get_column_names_for_component_types(self.schema(), component_types)

        # Use filter_contents with the column names
        if not all_column_names:
            # No matching columns - return empty view by filtering to non-existent entity path
            # Note: Empty list would return ALL columns, so we use a path that won't match anything
            return self.filter_contents(["/__rerun_internal__/non_existent"])

        return self.filter_contents(all_column_names)

    def reader(
        self,
        index: str | None,
        *,
        include_semantically_empty_columns: bool = False,
        include_tombstone_columns: bool = False,
        fill_latest_at: bool = False,
        using_index_values: IndexValuesLike | None = None,
    ) -> datafusion.DataFrame:
        """
        Create a reader over this dataset.

        Returns a DataFusion DataFrame.

        Server side filters
        -------------------

        The returned DataFrame supports server side filtering for both `rerun_segment_id`
        and the index (timeline) column, which can greatly improve performance. For
        example, the following filters will effectively be handled by the Rerun server.

        ```python
        dataset.reader(index="real_time").filter(col("rerun_segment_id") == "aabbccddee")
        dataset.reader(index="real_time").filter(col("real_time") == "1234567890")
        dataset.reader(index="real_time").filter(
            (col("rerun_segment_id") == "aabbccddee") & (col("real_time") == "1234567890")
        )
        ```

        Parameters
        ----------
        index : str | None
            The index (timeline) to use for the view.
            Pass `None` to read only static data.
        include_semantically_empty_columns : bool
            Whether to include columns that are semantically empty.
        include_tombstone_columns : bool
            Whether to include tombstone columns.
        fill_latest_at : bool
            Whether to fill null values with the latest valid data.
        using_index_values : IndexValuesLike | None
            If provided, specifies the exact index values to sample for all segments.
            Can be a numpy array (datetime64[ns] or int64), a pyarrow Array, or a sequence.
            Use with `fill_latest_at=True` to populate rows with the most recent data.

        Returns
        -------
        datafusion.DataFrame
            A DataFusion DataFrame.

        """

        return self.filter_contents(["/**"]).reader(
            index=index,
            include_semantically_empty_columns=include_semantically_empty_columns,
            include_tombstone_columns=include_tombstone_columns,
            fill_latest_at=fill_latest_at,
            using_index_values=using_index_values,
        )

    def create_fts_search_index(
        self,
        *,
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
        time_index: IndexColumnSelector,
        store_position: bool = False,
        base_tokenizer: str = "simple",
    ) -> None:
        """Create a full-text search index on the given column."""

        return self._internal.create_fts_search_index(
            column=column,
            time_index=time_index,
            store_position=store_position,
            base_tokenizer=base_tokenizer,
        )

    def create_vector_search_index(
        self,
        *,
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
        time_index: IndexColumnSelector,
        target_partition_num_rows: int | None = None,
        num_sub_vectors: int = 16,
        distance_metric: VectorDistanceMetric | str = "Cosine",
    ) -> IndexingResult:
        """
        Create a vector index on the given column.

        This will enable indexing and build the vector index over all existing values
        in the specified component column.

        Results can be retrieved using the `search_vector` API, which will include
        the time-point on the indexed timeline.

        Only one index can be created per component column -- executing this a second
        time for the same component column will replace the existing index.

        Parameters
        ----------
        column
            The component column to create the index on.
        time_index
            Which timeline this index will map to.
        target_partition_num_rows
            The target size (in number of rows) for each partition.
            The underlying indexer (lance) will pick a default when no value
            is specified - today this is 8192. It will also cap the
            maximum number of partitions independently of this setting - currently
            4096.
        num_sub_vectors
            The number of sub-vectors to use when building the index.
        distance_metric
            The distance metric to use for the index. ("L2", "Cosine", "Dot", "Hamming")

        """

        return self._internal.create_vector_search_index(
            column=column,
            time_index=time_index,
            target_partition_num_rows=target_partition_num_rows,
            num_sub_vectors=num_sub_vectors,
            distance_metric=distance_metric,
        )

    def list_search_indexes(self) -> list[IndexingResult]:
        """List all user-defined indexes in this dataset."""

        return self._internal.list_search_indexes()

    def delete_search_indexes(
        self,
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
    ) -> list[IndexConfig]:
        """Deletes all user-defined indexes for the specified column."""

        return self._internal.delete_search_indexes(column)

    def search_fts(
        self,
        query: str,
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
    ) -> datafusion.DataFrame:
        """Search the dataset using a full-text search query."""

        return self._internal.search_fts(query, column)

    def search_vector(
        self,
        query: Any,  # VectorLike
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
        top_k: int,
    ) -> datafusion.DataFrame:
        """Search the dataset using a vector search query."""

        return self._internal.search_vector(query, column, top_k)

    def do_maintenance(  # noqa: PLR0917
        self,
        optimize_indexes: bool = False,
        retrain_indexes: bool = False,
        compact_fragments: bool = False,
        cleanup_before: datetime | None = None,
        unsafe_allow_recent_cleanup: bool = False,
    ) -> None:
        """Perform maintenance tasks on the datasets."""

        return self._internal.do_maintenance(
            optimize_indexes, retrain_indexes, compact_fragments, cleanup_before, unsafe_allow_recent_cleanup
        )


class DatasetView:
    """
    A filtered view over a dataset in the catalog.

    A `DatasetView` provides lazy filtering over a dataset's segments and entity paths.
    Filters are composed lazily and only applied when data is actually read.

    Create a `DatasetView` by calling `filter_segments()` or `filter_contents()` on a
    `DatasetEntry`.

    Examples
    --------
    ```python
    # Filter to specific segments
    view = dataset.filter_segments(["recording_0", "recording_1"])

    # Filter to specific entity paths
    view = dataset.filter_contents(["/points/**"])

    # Chain filters
    view = dataset.filter_segments(["recording_0"]).filter_contents(["/points/**"])

    # Read data
    df = view.reader(index="timeline")
    ```

    """

    def __init__(self, internal: DatasetViewInternal) -> None:
        """
        Create a new DatasetView wrapper.

        Parameters
        ----------
        internal : DatasetViewInternal
            The internal Rust-side DatasetView object.

        """
        self._internal = internal

    @property
    def dataset(self) -> DatasetEntry:
        return DatasetEntry(self._internal.dataset)

    def schema(self) -> Schema:
        """
        Return the filtered schema for this view.

        The schema reflects any content filters applied to the view.

        Returns
        -------
        Schema
            The filtered schema.

        """
        from ._schema import Schema

        return Schema(self._internal.schema())

    def arrow_schema(self) -> pa.Schema:
        """
        Return the filtered Arrow schema for this view.

        Returns
        -------
        pa.Schema
            The filtered Arrow schema.

        """
        return self._internal.arrow_schema()

    def segment_ids(self) -> list[str]:
        """
        Return the segment IDs for this view.

        If segment filters have been applied, only matching segments are returned.

        Returns
        -------
        list[str]
            The list of segment IDs.

        """
        return self._internal.segment_ids()

    def segment_table(
        self,
        join_meta: TableEntry | datafusion.DataFrame | None = None,
        join_key: str = "rerun_segment_id",
    ) -> datafusion.DataFrame:
        """
        Return the segment table as a DataFusion DataFrame.

        The segment table contains metadata about each segment in the dataset,
        including segment IDs, layer names, storage URLs, and size information.

        Only segments matching this view's filters are included.

        Parameters
        ----------
        join_meta
            Optional metadata table or DataFrame to join with the segment table.
            If a `TableEntry` is provided, it will be converted to a DataFrame
            using `reader()`.
        join_key
            The column name to use for joining, defaults to "rerun_segment_id".
            Both the segment table and `join_meta` must contain this column.

        Returns
        -------
        datafusion.DataFrame
            The segment metadata table, optionally joined with `join_meta`.

        """
        segment_table_df = self.dataset.segment_table(join_meta, join_key)

        filtered_segment_ids = self._internal.filtered_segment_ids
        if filtered_segment_ids is not None:
            from datafusion import col, functions as F, literal

            segment_table_df = segment_table_df.filter(
                F.in_list(col("rerun_segment_id"), [literal(seg) for seg in filtered_segment_ids])
            )

        return segment_table_df

    def reader(
        self,
        index: str | None,
        *,
        include_semantically_empty_columns: bool = False,
        include_tombstone_columns: bool = False,
        fill_latest_at: bool = False,
        using_index_values: IndexValuesLike | None = None,
    ) -> datafusion.DataFrame:
        """
        Create a reader over this DatasetView.

        Returns a DataFusion DataFrame.

        Server side filters
        -------------------

        The returned DataFrame supports server side filtering for both `rerun_segment_id`
        and the index (timeline) column, which can greatly improve performance. For
        example, the following filters will effectively be handled by the Rerun server.

        ```python
        dataset.reader(index="real_time").filter(col("rerun_segment_id") == "aabbccddee")
        dataset.reader(index="real_time").filter(col("real_time") == "1234567890")
        dataset.reader(index="real_time").filter(
            (col("rerun_segment_id") == "aabbccddee") & (col("real_time") == "1234567890")
        )
        ```

        Parameters
        ----------
        index
            The index (timeline) to use for the view.
            Pass `None` to read only static data.
        include_semantically_empty_columns
            Whether to include columns that are semantically empty.
        include_tombstone_columns
            Whether to include tombstone columns.
        fill_latest_at
            Whether to fill null values with the latest valid data.
        using_index_values
            If provided, specifies the exact index values to sample for all segments.
            Can be a numpy array (datetime64[ns] or int64), a pyarrow Array, or a sequence.
            Use with `fill_latest_at=True` to populate rows with the most recent data.

        Returns
        -------
        A DataFusion DataFrame.

        """
        return self._internal.reader(
            index=index,
            include_semantically_empty_columns=include_semantically_empty_columns,
            include_tombstone_columns=include_tombstone_columns,
            fill_latest_at=fill_latest_at,
            using_index_values=using_index_values,
        )

    def filter_segments(self, segment_ids: str | Sequence[str] | datafusion.DataFrame) -> DatasetView:
        """
        Return a new DatasetView filtered to the given segment IDs.

        Filters are composed: if this view already has a segment filter,
        the result is the intersection of both filters.

        Parameters
        ----------
        segment_ids : str | Sequence[str] | datafusion.DataFrame
            A segment ID string, a list of segment ID strings, or a DataFusion DataFrame
            with a column named 'rerun_segment_id'.

        Returns
        -------
        DatasetView
            A new view filtered to the given segments.

        """
        import datafusion

        if isinstance(segment_ids, str):
            segment_ids = [segment_ids]
        elif isinstance(segment_ids, datafusion.DataFrame):
            segment_ids = segment_ids.select("rerun_segment_id").to_pydict()["rerun_segment_id"]

        return DatasetView(self._internal.filter_segments(list(segment_ids)))

    def filter_contents(
        self, exprs: str | Sequence[str]
    ) -> DatasetView:
        """
        Return a new DatasetView filtered to the given entity paths or specific component columns.

        Filters are composed: if this view already has filters, the result is the intersection
        of the existing filters and the new content filter.

        This method accepts both entity path patterns and component column selectors:
        - **Entity path patterns** (no colon): Include all components at matching entities
        - **Component column selectors** (with colon): Include only specific components

        The format automatically detects which mode to use based on the presence of `:` in the string.

        Entity path expressions support wildcards:
        - `"/points/**"` matches all entities under /points
        - `"-/text/**"` excludes all entities under /text

        Component column selectors use the format: `"entity_path:component"`
        - Example: `"/world/points:Position3D"`
        - This matches the format used by blueprint APIs (ComponentColumnSelector)

        Parameters
        ----------
        exprs : str | Sequence[str]
            Can be:
            - Single string (entity path or column selector)
            - List of strings (can mix entity paths and column selectors)

            Entity path examples:
            - `"/points/**"` - All components at entities under /points
            - `"/world/camera"` - All components at /world/camera
            - `"-/text/**"` - Exclude entities under /text

            Column selector examples:
            - `"/world/points:Position3D"` - Only Position3D component at /world/points
            - `"/world/points:Points3D:positions"` - Specific archetype component

        Returns
        -------
        DatasetView
            A new view filtered to the matching entity paths/components.

        Notes
        -----
        Edge cases and special behavior:

        - **Property paths**: Entity paths starting with `"property:"` or containing
          `"/__properties:"` are treated as entity paths (not column selectors), even
          though they contain colons.
        - **Mixing modes**: Cannot mix entity path patterns and column selectors in the
          same call. Use either entity paths (wildcards allowed) OR column selectors
          (exact paths only), not both. Raises `ValueError` if mixed.
        - **Empty filter**: Passing an empty list `[]` returns all columns. To get an
          empty view, filter to a non-existent path.
        - **Invalid selectors**: Invalid column selector strings raise `ValueError` with
          a clear error message.
        - **Filter composition**: When called on a DatasetView, filters are composed
          (intersected) with any existing filters.

        See Also
        --------
        filter_archetypes : Filter to specific archetype types (higher-level filtering)
        filter_component_types : Filter to specific component types (higher-level filtering)
        schema.column_names_for : Get column names for filtering programmatically

        """
        if isinstance(exprs, str):
            exprs = [exprs]

        # Separate entity paths from column selectors based on presence of ":"
        entity_paths = []
        column_selectors = []

        for expr in exprs:
            # Property paths contain ":" but are entity paths, not column selectors
            # Column selectors have format "entity_path:component"
            # Property paths start with "property:" or contain "/__properties:"
            is_property_path = expr.startswith("property:") or "/__properties:" in expr

            if ":" in expr and not expr.startswith("-") and not is_property_path:
                # It's a column selector (has colon, not a negation, and not a property path)
                column_selectors.append(expr)
            else:
                # It's an entity path pattern (no colon, is a negation, or is a property path)
                entity_paths.append(expr)

        # If we have column selectors, use component-level filtering
        if column_selectors:
            if entity_paths:
                # Mixed mode not supported: raise clear error
                raise ValueError(
                    "Cannot mix entity path patterns and column selectors in the same filter. "
                    "Please use either entity paths (no colon) or column selectors (with colon), not both."
                )
            return DatasetView(self._internal.filter_contents_columns(column_selectors))

        # Otherwise use entity-path-only filtering (current behavior)
        return DatasetView(self._internal.filter_contents(entity_paths))

    def filter_archetypes(
        self,
        archetypes: ArchetypeSpec | Sequence[ArchetypeSpec],  # type: ignore[name-defined]
    ) -> DatasetView:
        """
        Return a new DatasetView filtered to the given archetypes.

        Filters are composed: if this view already has filters, the result is the intersection
        of the existing filters and the new archetype filter.

        Parameters
        ----------
        archetypes : str | Type[Archetype] | Sequence[str | Type[Archetype]]
            Archetype or list of archetypes. Can be either:
            - Fully-qualified string (e.g., "rerun.archetypes.Points3D")
            - Archetype type (e.g., rr.Points3D)
            - List of either

        Returns
        -------
        DatasetView
            A new view filtered to the matching archetypes.

        Examples
        --------
        Filter to Points3D using type:

        ```python
        import rerun as rr
        dataset = rr.catalog.Dataset(...)
        view = dataset.filter_archetypes(rr.Points3D)
        ```

        Filter to multiple archetypes:

        ```python
        view = dataset.filter_archetypes([rr.Points3D, rr.Transform3D])
        ```

        Filter using fully-qualified string:

        ```python
        view = dataset.filter_archetypes("rerun.archetypes.Points3D")
        ```

        Chain with other filters:

        ```python
        view = dataset.filter_segments(["seg1"]).filter_archetypes(rr.Points3D)
        ```

        """
        # Normalize to a list
        if not isinstance(archetypes, Sequence) or isinstance(archetypes, str):
            archetypes = [archetypes]  # type: ignore[list-item]

        # Get column names using shared helper
        all_column_names = _get_column_names_for_archetypes(self.schema(), archetypes)

        # If no columns found, return an empty view
        if not all_column_names:
            # Note: Empty list would return ALL columns, so we use a path that won't match anything
            return self.filter_contents(["/__rerun_internal__/non_existent"])

        # Filter by the collected column names
        return self.filter_contents(all_column_names)

    def filter_component_types(
        self,
        component_types: ComponentTypeSpec | Sequence[ComponentTypeSpec],  # type: ignore[name-defined]
    ) -> DatasetView:
        """
        Return a new DatasetView filtered to the given component types.

        Filters are composed: if this view already has filters, the result is the intersection
        of the existing filters and the new component type filter.

        Parameters
        ----------
        component_types : str | Type[ComponentMixin] | Sequence[str | Type[ComponentMixin]]
            Component type or list of component types. Can be either:
            - Fully-qualified string (e.g., "rerun.components.Position3D")
            - ComponentMixin type (e.g., rr.components.Position3D)
            - List of either

        Returns
        -------
        DatasetView
            A new view filtered to the matching component types.

        Examples
        --------
        Filter to Position3D using type:

        ```python
        import rerun as rr
        from rerun import components

        dataset = rr.catalog.Dataset(...)
        view = dataset.filter_component_types(components.Position3D)
        ```

        Filter to multiple component types:

        ```python
        view = dataset.filter_component_types([components.Position3D, components.Color])
        ```

        Filter using fully-qualified string:

        ```python
        view = dataset.filter_component_types("rerun.components.Position3D")
        ```

        Chain with other filters:

        ```python
        view = dataset.filter_segments(["seg1"]).filter_component_types(components.Position3D)
        ```

        """
        # Normalize to a list
        if not isinstance(component_types, Sequence) or isinstance(component_types, str):
            component_types = [component_types]  # type: ignore[list-item]

        # Get column names using shared helper
        all_column_names = _get_column_names_for_component_types(self.schema(), component_types)

        # If no columns found, return an empty view
        if not all_column_names:
            # Note: Empty list would return ALL columns, so we use a path that won't match anything
            return self.filter_contents(["/__rerun_internal__/non_existent"])

        # Filter by the collected column names
        return self.filter_contents(all_column_names)

    def __repr__(self) -> str:
        """Return a string representation of the DatasetView."""

        dataset_str = str(self.dataset)

        filter_segment_ids = self._internal.filtered_segment_ids
        if filter_segment_ids is not None:
            segment_str = f"{len(filter_segment_ids)} segments"
        else:
            segment_str = "all segments"

        content_filters = self._internal.content_filters
        if content_filters:
            content_str = f"content_filters={content_filters!r}"
        else:
            content_str = "no content filter"

        return f"DatasetView({dataset_str}, {segment_str}, {content_str})"


class TableEntry(Entry[TableEntryInternal]):
    """
    A table entry in the catalog.

    Note: this object acts as a table provider for DataFusion.
    """

    def __datafusion_table_provider__(self) -> Any:
        """Returns a DataFusion table provider capsule."""

        return self._internal.__datafusion_table_provider__()

    def reader(self) -> datafusion.DataFrame:
        """Registers the table with the DataFusion context and return a DataFrame."""

        return self._internal.reader()

    def to_arrow_reader(self) -> pa.RecordBatchReader:
        """Convert this table to a [`pyarrow.RecordBatchReader`][]."""

        return self._internal.to_arrow_reader()

    @property
    def storage_url(self) -> str:
        """The table's storage URL."""

        return self._internal.storage_url

    def arrow_schema(self) -> pa.Schema:
        """Returns the Arrow schema of the table."""

        return self.reader().schema()

    # ---

    def append(
        self,
        batches: _BatchesType | None = None,
        **named_params: Any,
    ) -> None:
        """
        Append to the Table.

        Parameters
        ----------
        batches
            Arrow data to append to the table. Can be a RecordBatchReader, a single RecordBatch, a list of
            RecordBatches, or a list of lists of RecordBatches (as returned by `datafusion.DataFrame.collect()`).
        **named_params
            Each named parameter corresponds to a column in the table.

        """
        self._write(batches, named_params, TableInsertModeInternal.APPEND)

    def overwrite(
        self,
        batches: _BatchesType | None = None,
        **named_params: Any,
    ) -> None:
        """
        Overwrite the Table with new data.

        Parameters
        ----------
        batches
            Arrow data to overwrite the table with. Can be a RecordBatchReader, a single RecordBatch, a list of
            RecordBatches, or a list of lists of RecordBatches (as returned by `datafusion.DataFrame.collect()`).

        **named_params
            Each named parameter corresponds to a column in the table.

        """
        self._write(batches, named_params, TableInsertModeInternal.OVERWRITE)

    def upsert(
        self,
        batches: _BatchesType | None = None,
        **named_params: Any,
    ) -> None:
        """
        Upsert data into the Table.

        To use upsert, the table must contain a column with the metadata:
        ```
            {"rerun:is_table_index" = "true"}
        ```

        Any row with a matching index value will have the new data inserted.
        Any row without a matching index value will be appended as a new row.

        Parameters
        ----------
        batches
            Arrow data to upsert into the table. Can be a RecordBatchReader, a single RecordBatch, a list of
            RecordBatches, or a list of lists of RecordBatches (as returned by `datafusion.DataFrame.collect()`).
        **named_params
            Each named parameter corresponds to a column in the table

        """
        self._write(batches, named_params, TableInsertModeInternal.REPLACE)

    def _write(
        self,
        batches: _BatchesType | None,
        named_params: dict[str, Any],
        insert_mode: TableInsertModeInternal,
    ) -> None:
        """Internal helper that implements append/overwrite/upsert."""
        if batches is not None and len(named_params) > 0:
            raise TypeError("Cannot pass both batches and named parameters. Use one or the other.")

        if batches is not None:
            self._write_batches(batches, insert_mode=insert_mode)
        else:
            self._write_named_params(named_params, insert_mode=insert_mode)

    def _write_batches(
        self,
        batches: _BatchesType,
        insert_mode: TableInsertModeInternal,
    ) -> None:
        """Internal helper to write batches to the table."""
        # If already a RecordBatchReader, pass it directly
        if isinstance(batches, RecordBatchReader):
            self._internal.write_batches(batches, insert_mode=insert_mode)
            return

        flat_batches = self._flatten_batches(batches)
        if len(flat_batches) == 0:
            return
        schema = flat_batches[0].schema
        reader = RecordBatchReader.from_batches(schema, flat_batches)
        self._internal.write_batches(reader, insert_mode=insert_mode)

    def _flatten_batches(
        self,
        batches: pa.RecordBatch | Sequence[pa.RecordBatch] | Sequence[Sequence[pa.RecordBatch]],
    ) -> list[pa.RecordBatch]:
        """Flatten batches to a list of RecordBatches."""
        if isinstance(batches, pa.RecordBatch):
            return [batches]

        result = []
        for item in batches:
            if isinstance(item, pa.RecordBatch):
                result.append(item)
            elif isinstance(item, Sequence):
                result.extend(item)
            else:
                raise TypeError(f"Unexpected type: {type(item)}")
        return result

    def _write_named_params(
        self,
        named_params: dict[str, Any],
        insert_mode: TableInsertModeInternal,
    ) -> None:
        """Internal helper to write named parameters to the table."""
        batch = self._python_objects_to_record_batch(self.arrow_schema(), named_params)
        if batch is not None:
            reader = RecordBatchReader.from_batches(batch.schema, [batch])
            self._internal.write_batches(reader, insert_mode=insert_mode)

    def _python_objects_to_record_batch(self, schema: pa.Schema, named_params: dict[str, Any]) -> pa.RecordBatch:
        cast_params = {}
        expected_len = None

        for name, value in named_params.items():
            field = schema.field(name)
            if field is None:
                raise ValueError(f"Column {name} does not exist in table")

            if isinstance(value, str):
                value = [value]

            try:
                cast_value = pa.array(value, type=field.type)
            except TypeError:
                cast_value = pa.array([value], type=field.type)

            cast_params[name] = cast_value

            if expected_len is None:
                expected_len = len(cast_value)
            else:
                if len(cast_value) != expected_len:
                    raise ValueError("Columns have mismatched number of rows")

        if expected_len is None or expected_len == 0:
            return

        columns = []
        for field in schema:
            if field.name in cast_params:
                columns.append(cast_params[field.name])
            else:
                columns.append(pa.array([None] * expected_len, type=field.type))

        return pa.RecordBatch.from_arrays(columns, schema=schema)
