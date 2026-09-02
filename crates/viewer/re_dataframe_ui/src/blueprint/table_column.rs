use re_log_types::{EntityPath, EntityPathPart};
use re_sdk_types::blueprint::components::{
    ColumnName, Editable, IncludedContent, TableCellKind, TableLayoutKind,
};
use re_sdk_types::{blueprint::archetypes, components::Visible};
use re_sorbet::{BatchType, ColumnDescriptorRef};
use re_types_core::Archetype as _;
use re_viewer_context::{AppBlueprintCtx, BlueprintContext as _};

use crate::datafusion_table_widget::{DataColumns, TableColumnHeuristic};
use crate::display_record_batch::DisplayColumn;

/// Resolved runtime configuration for a table column.
///
/// Values can be loaded from [`archetypes::TableColumn`] and completed from the underlying table
/// schema and runtime heuristics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumn<'a> {
    /// The physical name of the source column where data is read from.
    source: ColumnName,

    /// Sorbet column descriptor if we were able to match the column up with a sorbet column.
    // TODO(andreas): this is always present after resolve, can we make it non-optional?
    sorbet_column_descriptor: Option<ColumnDescriptorRef<'a>>,

    /// Display name if dictated by blueprint.
    name: Option<String>,

    /// Whether the column's values can be edited.
    ///
    /// This is false when the blueprint does not configure the field.
    editable: bool,

    /// Whether the column is visible in the table layout.
    ///
    /// When unset, the table kind and layout determine visibility.
    visible: Option<bool>,

    /// How to render the column's values.
    ///
    /// This is [`TableCellKind::Auto`] when the blueprint does not configure the field.
    cell_kind: TableCellKind,

    /// Views rendered when this is a preview column, in display order.
    /// (extracted from [`archetypes::TableColumnPreview`])
    preview_views: Vec<EntityPath>,
}

/// Retain configured columns in their explicit order, then append data columns in their default order.
///
/// `None` leaves visibility unresolved so caller heuristics and the layout fallback can decide.
/// Columns with the same default priority retain their schema order.
pub fn resolve_columns<'a>(
    blueprint: &AppBlueprintCtx<'_>,
    layout_kind: TableLayoutKind,
    columns: &mut Vec<TableColumn<'a>>,
    data_columns: &'a DataColumns<'a>,
    additional_column_heuristics: &TableColumnHeuristic<'_>,
) {
    columns.retain_mut(|column| {
        let Some((_, data_column)) = data_columns.find_by_physical_name(column.physical_name())
        else {
            return false;
        };

        column.sorbet_column_descriptor = Some(data_column.desc.clone());
        *column = additional_column_heuristics(&data_column.desc, column.clone());
        true
    });

    let mut unconfigured_columns = data_columns
        .iter()
        .filter(|data_column| {
            columns
                .iter()
                .all(|column| column.physical_name() != data_column.physical_name())
        })
        .collect::<Vec<_>>();
    unconfigured_columns
        .sort_by_key(|column| TableColumn::default_order_priority(column.physical_name()));

    columns.extend(unconfigured_columns.into_iter().map(|data_column| {
        let mut column =
            TableColumn::load(data_column.physical_name().clone(), blueprint, layout_kind);
        column.sorbet_column_descriptor = Some(data_column.desc.clone()); // TODO(andreas): odd to fill this after the fact.
        column = additional_column_heuristics(&data_column.desc, column);

        let default_visibility = match layout_kind {
            TableLayoutKind::Table => true,
            TableLayoutKind::Cards => false,
        };
        column.with_default_visibility(default_visibility)
    }));
}

impl<'a> TableColumn<'a> {
    pub fn default_order_priority(column_name: &ColumnName) -> u8 {
        match column_name.as_str() {
            "name" | "rerun_segment_id" => 0,
            "link" | "recording link" => 1,
            _ => 2,
        }
    }

    /// Returns the entity path for a source column's configuration in one layout.
    ///
    /// `EntityPathPart` keeps arbitrary physical column names within one path part.
    pub fn blueprint_path(layout_kind: TableLayoutKind, source: &ColumnName) -> EntityPath {
        let base = match layout_kind {
            TableLayoutKind::Table => EntityPath::from("/table/layouts/table/columns"),
            TableLayoutKind::Cards => EntityPath::from("/table/layouts/cards/fields"),
        };
        base / EntityPathPart::new(source.as_str())
    }

    /// Load a column configuration for `source` from its layout-specific entity.
    pub fn load(
        source: ColumnName,
        blueprint: &AppBlueprintCtx<'_>,
        layout_kind: TableLayoutKind,
    ) -> Self {
        let results = blueprint.latest_at_in_current_blueprint(
            &Self::blueprint_path(layout_kind, &source),
            std::iter::chain(
                archetypes::TableColumn::all_component_identifiers(),
                archetypes::TableColumnPreview::all_component_identifiers(),
            ),
        );

        Self {
            source,
            sorbet_column_descriptor: None,

            name: results
                .component_mono::<re_sdk_types::components::Name>(
                    archetypes::TableColumn::descriptor_name().component,
                )
                .map(|name| name.0.to_string()),
            editable: results
                .component_mono::<Editable>(
                    archetypes::TableColumn::descriptor_editable().component,
                )
                .is_some_and(|editable| *editable.0),
            visible: results
                .component_mono::<Visible>(archetypes::TableColumn::descriptor_visible().component)
                .map(|visible| *visible.0),
            cell_kind: results
                .component_mono::<TableCellKind>(
                    archetypes::TableColumn::descriptor_cell_kind().component,
                )
                .unwrap_or_default(),
            preview_views: results
                .component_batch::<IncludedContent>(
                    archetypes::TableColumnPreview::descriptor_views().component,
                )
                .unwrap_or_default()
                .iter()
                .map(|content| content.0.clone().into())
                .collect(),
        }
    }

    /// Create a default column from its physical name and Sorbet descriptor.
    pub fn new_without_blueprint(source: ColumnName, desc: ColumnDescriptorRef<'a>) -> Self {
        Self {
            source,
            name: None,
            sorbet_column_descriptor: Some(desc),
            editable: false,
            visible: None,
            cell_kind: TableCellKind::Auto,
            preview_views: Vec::new(),
        }
    }

    /// Write a column's visibility to its layout-specific blueprint entity.
    ///
    /// This will be in effect next frame.
    pub fn save_visibility(
        blueprint: &AppBlueprintCtx<'_>,
        source: &ColumnName,
        layout_kind: TableLayoutKind,
        visible: bool,
    ) {
        blueprint.save_blueprint_archetype(
            Self::blueprint_path(layout_kind, source),
            &archetypes::TableColumn::update_fields().with_visible(visible),
        );
    }

    /// Set the display name unless the stored blueprint configured one.
    pub fn with_default_display_name(mut self, name: impl Into<String>) -> Self {
        if self.name.is_none() {
            self.name = Some(name.into());
        }
        self
    }

    /// Set visibility unless the stored blueprint configured it.
    pub fn with_default_visibility(mut self, visible: bool) -> Self {
        self.visible.get_or_insert(visible);
        self
    }

    /// Set the cell kind unless the stored blueprint configured a non-auto kind.
    pub fn with_default_cell_kind(mut self, cell_kind: TableCellKind) -> Self {
        if self.cell_kind == TableCellKind::Auto {
            self.cell_kind = cell_kind;
        }
        self
    }

    /// Returns whether this column is visible in the given layout.
    pub fn is_visible(&self, layout_kind: TableLayoutKind) -> bool {
        self.visible.unwrap_or_else(|| match layout_kind {
            TableLayoutKind::Table => !self.source.as_str().starts_with("rerun_"),
            TableLayoutKind::Cards => false,
        })
    }

    /// Returns the configured display name, or the name inferred from the column descriptor.
    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| {
            self.sorbet_column_descriptor
                .as_ref()
                .map(default_display_name_for_column)
                .unwrap_or_else(|| self.source.0.to_string())
        })
    }

    /// The data source of the column in the unaltered, pre-migration schema.
    ///
    /// Also known as "source".
    #[doc(alias = "source")]
    pub fn physical_name(&self) -> &ColumnName {
        &self.source
    }

    /// Resolves how cells of this column should be displayed based on the actual value in the data.
    ///
    /// Explicit blueprint configuration takes precedence over value-based inference.
    pub fn value_resolved_cell_kind(
        &self,
        display_column: Option<&DisplayColumn>,
        row: usize,
    ) -> TableCellKind {
        if self.cell_kind != TableCellKind::Auto {
            return self.cell_kind;
        }

        if let Some(ColumnDescriptorRef::Component(component)) = self.sorbet_column_descriptor
            && component.component == "entry_kind"
        {
            TableCellKind::EntryKind
        } else if let Some(DisplayColumn::Component(component)) = display_column
            && component.is_image(row)
        {
            TableCellKind::Thumbnail
        } else {
            TableCellKind::Auto
        }
    }

    /// Returns the cell kind before value-based inference.
    pub fn configured_cell_kind(&self) -> TableCellKind {
        self.cell_kind
    }

    /// Whether the blueprint requests editing for this column.
    ///
    /// It may not be actually editable depending on the context and various restrictions we have.
    pub fn editable(&self) -> bool {
        self.editable
    }

    /// Blueprint paths at which the preview views for this column are configured.
    pub fn preview_views_blueprint_paths(&self) -> &[EntityPath] {
        &self.preview_views
    }
}

fn default_display_name_for_column(desc: &ColumnDescriptorRef<'_>) -> String {
    match desc {
        ColumnDescriptorRef::RowId(_) | ColumnDescriptorRef::Time(_) => desc.display_name(),

        ColumnDescriptorRef::Component(desc) => {
            if desc.entity_path == EntityPath::root() {
                desc.column_name(BatchType::Chunk)
            } else {
                desc.column_name(BatchType::Dataframe)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blueprint_path_keeps_column_name_in_one_part() {
        let source = "property:episode/task name#value";
        let path = TableColumn::blueprint_path(TableLayoutKind::Table, &source.into());
        assert_eq!(
            path.to_string(),
            r"/table/layouts/table/columns/property\:episode\/task\ name\#value"
        );
        assert_eq!(EntityPath::parse_strict(&path.to_string()).unwrap(), path);
    }
}
