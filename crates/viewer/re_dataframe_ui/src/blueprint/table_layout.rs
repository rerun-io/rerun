use re_sdk_types::blueprint::archetypes;
use re_sdk_types::blueprint::components::{ColumnName, TableLayoutKind};
use re_types_core::Archetype as _;
use re_viewer_context::{AppBlueprintCtx, BlueprintContext as _};

use crate::blueprint::TableColumn;
use crate::blueprint::table_column::resolve_columns;
use crate::datafusion_table_widget::{DataColumns, TableColumnHeuristic};

/// Resolved runtime configuration for displaying table records as rows and columns.
///
/// Every source column is loaded from the table schema.
/// `column_order` makes columns visible by default and puts them first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableLayout<'a> {
    /// Columns in display order.
    pub columns: Vec<TableColumn<'a>>,
}

impl<'a> TableLayout<'a> {
    pub fn load_and_resolve(
        blueprint: &AppBlueprintCtx<'_>,
        data_columns: &'a DataColumns<'a>,
        additional_column_heuristics: &TableColumnHeuristic<'_>,
    ) -> Self {
        let results = blueprint.latest_at_in_current_blueprint(
            &"/table/layouts/table".into(),
            archetypes::TableLayout::all_component_identifiers(),
        );

        let column_names = results
            .component_batch::<ColumnName>(
                archetypes::TableLayout::descriptor_column_order().component,
            )
            .unwrap_or_default()
            .clone();

        let mut columns = column_names
            .into_iter()
            .map(|column_name| {
                TableColumn::load(column_name, blueprint, TableLayoutKind::Table)
                    .with_default_visibility(true)
            })
            .collect();

        resolve_columns(
            blueprint,
            TableLayoutKind::Table,
            &mut columns,
            data_columns,
            additional_column_heuristics,
        );

        Self { columns }
    }
}
