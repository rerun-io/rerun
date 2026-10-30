"""
Configure catalog table layouts, columns, and segment previews.

⚠️ **The `rerun.blueprint.table` namespace is _unstable_ and may change significantly.**
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Literal, cast

import rerun_bindings as bindings

from .._baseclasses import ComponentDescriptor, ComponentMixin
from .._log import escape_entity_path_part
from ..recording_stream import RecordingStream
from ._table_cells import PreviewCell as PreviewCell
from ._table_filters import (
    Filter as Filter,
    FilterColumn as FilterColumn,
    ListFilters as ListFilters,
    StringFilters as StringFilters,
    col as col,
)
from .archetypes import (
    CardLayout as CardLayoutArchetype,
    PreviewsConfig as PreviewsConfig,
    TableBlueprint as TableBlueprintArchetype,
    TableColumn as TableColumnArchetype,
    TableColumnPreview,
    TableLayout as TableLayoutArchetype,
)
from .encodings import ColumnType, SortBy as SortBy

if TYPE_CHECKING:
    from collections.abc import Sequence
    from pathlib import Path

    from .api import View
    from .components import ColumnDisplayModeLike

__all__ = [
    "CardLayout",
    "Column",
    "Filter",
    "FilterColumn",
    "ListFilters",
    "PreviewCell",
    "PreviewsConfig",
    "SortBy",
    "StringFilters",
    "TableBlueprint",
    "TableLayout",
    "col",
]


def _validate_optional_string(value: object, name: str, *, allow_empty: bool = True) -> None:
    if value is not None and not isinstance(value, str):
        raise TypeError(f"{name} must be a string or None")
    if not allow_empty and value == "":
        raise ValueError(f"{name} must not be empty")


def _validate_optional_bool(value: object, name: str) -> None:
    if value is not None and not isinstance(value, bool):
        raise TypeError(f"{name} must be a bool or None")


def _validate_unique_sources(columns: Sequence[Column], context: str) -> tuple[Column, ...]:
    result = tuple(columns)
    if not all(isinstance(column, Column) for column in result):
        raise TypeError(f"{context} only accepts Column values")

    sources = [column.source for column in result]
    if len(sources) != len(set(sources)):
        raise ValueError(f"{context} accepts each source column at most once")
    return result


@dataclass(frozen=True)
class Column:
    """
    Configure a source column's appearance and behavior in one layout.

    Parameters
    ----------
    source:
        Physical Arrow column name, not a display label.
    name:
        Display label, overriding `column_display_mode`.
        If unset, the label is derived from `source`.
    editable:
        Allow editing.
        If unset, the column is editable when its field in the table schema has `rerun:is_editable` metadata,
        e.g. from `rr.components.Flag.table_field("reviewed", editable=True)`.
        Requires write permission on a remote table with a column marked by `rerun:is_table_index` metadata.
        A cell can be edited when its component type has an editor.
        Boolean columns are shown as [`rerun.components.Bool`][] unless `column_type` is set, e.g. to [`rerun.components.Flag`][].
        Editable string and 64-bit float columns without a component type are shown as [`rerun.components.Text`][] and [`rerun.components.Scalar`][].
    column_type:
        The component the column's values are shown as.
        Takes a component class like [`rerun.components.Url`][] or [`rerun.components.Flag`][],
        a fully qualified component type name like `"rerun.components.Flag"`,
        or a [`ComponentDescriptor`][rerun.ComponentDescriptor] like `rr.TextDocument.descriptor_text()`
        for the UI of a specific archetype field.
        The column's Arrow datatype must match the datatype of the component type.
        [`PreviewCell`][rerun.blueprint.table.PreviewCell] shows the column as previews instead.
        If unset, the Viewer uses the component type of the source column, or infers one from the column's datatype.
    visible:
        Override visibility in this layout.
        Configured columns are visible by default.
        Set to `False` to hide a column while retaining its configuration and position.

    """

    source: str
    name: str | None = field(default=None, kw_only=True)
    editable: bool | None = field(default=None, kw_only=True)
    column_type: type[ComponentMixin] | str | ComponentDescriptor | PreviewCell | None = field(
        default=None, kw_only=True
    )
    visible: bool | None = field(default=None, kw_only=True)

    def __post_init__(self) -> None:
        if not isinstance(self.source, str):
            raise TypeError("Column.source must be a string")
        if not self.source:
            raise ValueError("Column.source must not be empty")
        _validate_optional_string(self.name, "Column.name")
        _validate_optional_bool(self.editable, "Column.editable")
        _validate_optional_bool(self.visible, "Column.visible")
        self._column_type()

    def _column_type(self) -> ColumnType | None:
        column_type = self.column_type
        if column_type is None or isinstance(column_type, PreviewCell):
            return None
        if isinstance(column_type, str):
            if not column_type:
                raise ValueError("Column.column_type must not be empty")
            return ColumnType(component_type_name=column_type)
        if isinstance(column_type, ComponentDescriptor):
            return ColumnType(component=column_type.component, component_type_name=column_type.component_type)
        if isinstance(column_type, type) and issubclass(column_type, ComponentMixin):  # type: ignore[redundant-expr]
            return ColumnType(component_type_name=column_type.component_type())
        raise TypeError(
            "Column.column_type must be a component class, a component type name, a ComponentDescriptor, or a PreviewCell"
        )

    def _log_to_stream(self, stream: RecordingStream, path: str) -> None:
        components = TableColumnArchetype(
            name=self.name,
            editable=self.editable,
            visible=self.visible,
            column_type=self._column_type(),
        ).as_component_batches()
        if components:
            stream.log(path, components)

        if isinstance(self.column_type, PreviewCell):
            stream.log(path, TableColumnPreview(views=[view.blueprint_path() for view in self.column_type.views]))


@dataclass(frozen=True, kw_only=True)
class TableLayout:
    """
    Display table records as rows and columns.

    Parameters
    ----------
    columns:
        Columns to configure, in display order, with each source listed at most once.
        Listed columns appear first and are visible unless `visible=False`.
        Unlisted columns follow in default order and retain Viewer visibility defaults.

    """

    columns: Sequence[Column] = ()

    def __post_init__(self) -> None:
        object.__setattr__(self, "columns", _validate_unique_sources(self.columns, "TableLayout.columns"))

    # TODO(RR-4810): Add `auto_visible_columns` after the type definitions model it.


@dataclass(frozen=True, kw_only=True)
class CardLayout:
    """
    Display table records as cards.

    Parameters
    ----------
    title:
        Physical Arrow column name to use for card titles.
        If unset or not found in the table, uses the first visible string field.
    link:
        Physical Arrow column name containing the target to open when a card is activated.
        If unset or not found in the table, uses the first visible field with a
        [`PreviewCell`][rerun.blueprint.table.PreviewCell].
    fields:
        Fields to configure, in display order, with each source listed at most once.
        Listed fields are visible unless `visible=False`; unlisted fields are hidden.
        The first visible field with `column_type` set to [`rerun.components.Flag`][] is shown in the card header
        instead of the labeled-field list.

    """

    title: str | None = None
    link: str | None = None
    fields: Sequence[Column] = ()

    def __post_init__(self) -> None:
        _validate_optional_string(self.title, "CardLayout.title", allow_empty=False)
        _validate_optional_string(self.link, "CardLayout.link", allow_empty=False)
        object.__setattr__(self, "fields", _validate_unique_sources(self.fields, "CardLayout.fields"))


@dataclass(frozen=True, kw_only=True)
class TableBlueprint:
    """
    Configure layouts and segment previews for a catalog table.

    This configures a table, not a Viewer viewport [`Blueprint`][rerun.blueprint.Blueprint].
    Save it as an `.rbl` file and register its server-accessible URI with
    `TableEntry.register_blueprint(uri)` or `DatasetEntry.register_blueprint(uri, segment_table=True)`.
    See [Configure table layouts and segment previews](https://rerun.io/docs/howto/visualization/configure-table-blueprints) for the full workflow.

    Parameters
    ----------
    table_layout:
        Row-and-column layout, available by default.
    card_layout:
        Card layout, disabled unless configured.
    default_layout:
        Initial layout: `"table"` or `"cards"`.
        If unset, uses cards when configured, otherwise the table layout.
        `"cards"` requires `card_layout`.
    column_display_mode:
        Column-name formatting in both layouts; defaults to compact formatting.
        Explicit `Column.name` labels take precedence.
    previews_config:
        Timeline, play state, and playback speed shared by every [`PreviewCell`][rerun.blueprint.table.PreviewCell] in both layouts.
        If no timeline is specified, one is picked automatically, preferring custom over built-in timelines.
        Previews start paused at 1x speed unless `play_state` or `playback_speed` is set.
    filters:
        Row filters, shared by both layouts.
        A row is only shown when all filters select it.
        Build filters for one column with [`col`][rerun.blueprint.table.col], which the Viewer shows as editable filters.
        Instead of a [`Filter`][rerun.blueprint.table.Filter], you can also provide a string with a raw SQL expression.
    sort_by:
        Source column the rows are sorted by, with an optional direction.
        Without a direction the viewer sorts ascending.
        Use [`SortBy`][rerun.blueprint.table.SortBy] to set the direction.
        If unset, rows keep the order of the data.

    """

    table_layout: TableLayout | None = None
    card_layout: CardLayout | None = None
    default_layout: Literal["table", "cards"] | None = None
    column_display_mode: ColumnDisplayModeLike | None = None
    previews_config: PreviewsConfig | None = None
    filters: Sequence[Filter | str] = ()
    sort_by: str | SortBy | None = None

    def __post_init__(self) -> None:
        if isinstance(self.filters, (str, Filter)):
            raise TypeError("TableBlueprint.filters must be a sequence of filters")
        object.__setattr__(
            self,
            "filters",
            tuple(entry if isinstance(entry, Filter) else Filter(entry) for entry in self.filters),
        )
        if self.table_layout is not None and not isinstance(self.table_layout, TableLayout):
            raise TypeError("TableBlueprint.table_layout must be a TableLayout or None")
        if self.card_layout is not None and not isinstance(self.card_layout, CardLayout):
            raise TypeError("TableBlueprint.card_layout must be a CardLayout or None")
        _validate_optional_string(self.default_layout, "TableBlueprint.default_layout")
        if self.default_layout not in (None, "table", "cards"):
            raise ValueError("TableBlueprint.default_layout must be 'table', 'cards', or None")
        if self.default_layout == "cards" and self.card_layout is None:
            raise ValueError("default_layout='cards' requires card_layout to be configured")
        if self.previews_config is not None and not isinstance(self.previews_config, PreviewsConfig):
            raise TypeError("TableBlueprint.previews_config must be a PreviewsConfig or None")
        if isinstance(self.sort_by, str):
            _validate_optional_string(self.sort_by, "TableBlueprint.sort_by", allow_empty=False)
            object.__setattr__(self, "sort_by", SortBy(self.sort_by))
        elif self.sort_by is not None and not isinstance(self.sort_by, SortBy):
            raise TypeError("TableBlueprint.sort_by must be a str, SortBy, or None")

    def save(self, application_id: str, path: str | Path | None = None) -> None:
        """
        Save this table blueprint to an `.rbl` file for catalog registration.

        Parameters
        ----------
        application_id:
            Application ID stored in the blueprint recording.
        path:
            Output file path; defaults to `<application_id>.rbl`.

        """
        if path is None:
            path = f"{application_id}.rbl"
        else:
            path = str(path)

        blueprint_stream = RecordingStream._from_native(
            bindings.new_blueprint(
                application_id=application_id,
                make_default=False,
                make_thread_default=False,
                default_enabled=True,
            ),
        )
        blueprint_stream.set_time("blueprint", sequence=0)
        self._log_to_stream(blueprint_stream)
        bindings.save_blueprint(path, blueprint_stream.to_native())

    def _log_to_stream(self, stream: RecordingStream) -> None:
        layouts = []
        if self.table_layout is not None:
            layouts.append(self.table_layout.columns)
        if self.card_layout is not None:
            layouts.append(self.card_layout.fields)
        views: dict[str, View] = {
            view.blueprint_path(): view
            for columns in layouts
            for column in columns
            if isinstance(column.column_type, PreviewCell)
            for view in column.column_type.views
        }

        for view in views.values():
            view._log_to_stream(stream)

        components = TableBlueprintArchetype(
            layout=self.default_layout,
            column_display_mode=self.column_display_mode,
            filters=[str(entry) for entry in self.filters] or None,
            # `__post_init__` converts a plain column name to `SortBy`.
            sort_by=cast("SortBy | None", self.sort_by),
        ).as_component_batches()
        if components:
            stream.log("table", components)
        if self.previews_config is not None:
            components = self.previews_config.as_component_batches()
            if components:
                stream.log("table", components)

        table_layout = self.table_layout if self.table_layout is not None else TableLayout()
        stream.log(
            "table/layouts/table",
            TableLayoutArchetype(column_order=[column.source for column in table_layout.columns]),
        )
        for column in table_layout.columns:
            source = escape_entity_path_part(column.source)
            column._log_to_stream(stream, f"table/layouts/table/columns/{source}")

        if self.card_layout is not None:
            stream.log(
                "table/layouts/cards",
                CardLayoutArchetype(
                    field_order=[field.source for field in self.card_layout.fields],
                    title=self.card_layout.title,
                    link=self.card_layout.link,
                ),
            )
            for field in self.card_layout.fields:
                source = escape_entity_path_part(field.source)
                field._log_to_stream(stream, f"table/layouts/cards/fields/{source}")
