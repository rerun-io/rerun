use re_log_types::TimelineName;
use re_sdk_types::blueprint::archetypes::{
    CardLayout as CardLayoutArchetype, TableBlueprint as TableBlueprintArchetype,
    TableLayout as TableLayoutArchetype,
};
use re_sdk_types::blueprint::components::{
    ColumnName, TableLayoutKind, TimelineName as BlueprintTimelineName,
};
use re_types_core::Archetype as _;
use re_viewer_context::{AppBlueprintCtx, BlueprintContext as _};

use crate::blueprint::{CardLayout, TableColumn, TableLayout};
use crate::datafusion_table_widget::{DataColumns, TableColumnHeuristic};

/// Shared configuration for preview columns in every layout.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreviewsConfig {
    pub timeline: Option<TimelineName>,
}

/// Resolved runtime configuration for presenting a table.
///
/// Initial values are loaded from registered blueprint archetypes and completed from the table's
/// schema and runtime heuristics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableBlueprint<'a> {
    /// The currently selected layout.
    ///
    /// If unset, defaults to card layout if available.
    /// `Cards` falls back to table layout when no [`CardLayout`] is configured.
    layout: Option<TableLayoutKind>,

    /// Shared configuration for preview columns in every layout.
    pub previews_config: PreviewsConfig,

    /// The configuration for displaying rows and columns.
    pub table_layout: TableLayout<'a>,

    /// The optional configuration for displaying records as cards.
    pub card_layout: Option<CardLayout<'a>>,
}

impl<'a> TableBlueprint<'a> {
    /// Load the table blueprint and resolve its layouts against the table schema and data.
    pub fn load_and_resolve(
        blueprint_ctx: &AppBlueprintCtx<'_>,
        data_columns: &'a DataColumns<'a>,
        additional_column_heuristics: &TableColumnHeuristic<'_>,
    ) -> Self {
        re_tracing::profile_function!();

        let results = blueprint_ctx.latest_at_in_current_blueprint(
            &"/table".into(),
            std::iter::chain(
                TableBlueprintArchetype::all_component_identifiers(),
                re_sdk_types::blueprint::archetypes::PreviewsConfig::all_component_identifiers(),
            ),
        );

        // TODO(andreas): Should we only resolve the active layout?
        Self {
            layout: results.component_mono(TableBlueprintArchetype::descriptor_layout().component),
            previews_config: PreviewsConfig {
                timeline: results
                    .component_mono::<BlueprintTimelineName>(
                        re_sdk_types::blueprint::archetypes::PreviewsConfig::descriptor_timeline()
                            .component,
                    )
                    .and_then(|timeline| TimelineName::try_new(timeline.as_str()).ok()),
            },
            table_layout: TableLayout::load_and_resolve(
                blueprint_ctx,
                data_columns,
                additional_column_heuristics,
            ),
            card_layout: CardLayout::load_and_resolve(
                blueprint_ctx,
                data_columns,
                additional_column_heuristics,
            ),
        }
    }

    /// Write the complete column order for a layout while preserving resolved visibility.
    ///
    /// This will be in effect next frame.
    pub fn save_column_order<'column>(
        blueprint_ctx: &AppBlueprintCtx<'_>,
        layout: TableLayoutKind,
        columns: impl IntoIterator<Item = &'column TableColumn<'column>>,
    ) {
        let column_order = columns.into_iter().map(|column| {
            if !column.is_visible(layout) {
                TableColumn::save_visibility(blueprint_ctx, column.physical_name(), layout, false);
            }
            column.physical_name().clone()
        });

        match layout {
            TableLayoutKind::Table => blueprint_ctx.save_blueprint_archetype(
                "/table/layouts/table".into(),
                &TableLayoutArchetype::update_fields().with_column_order(column_order),
            ),

            TableLayoutKind::Cards => blueprint_ctx.save_blueprint_archetype(
                "/table/layouts/cards".into(),
                &CardLayoutArchetype::update_fields().with_field_order(column_order),
            ),
        }
    }

    /// Returns the selected layout, resolved against the available layouts.
    pub fn layout(&self) -> TableLayoutKind {
        if self.card_layout.is_some() {
            // If cards are available show cards unless configured otherwise.
            self.layout.unwrap_or(TableLayoutKind::Cards)
        } else {
            // If no cards are available, ignore the configuration.
            TableLayoutKind::Table
        }
    }

    /// Write the selected layout.
    ///
    /// This will be in effect next frame.
    pub fn save_layout(blueprint_ctx: &AppBlueprintCtx<'_>, layout: TableLayoutKind) {
        blueprint_ctx.save_blueprint_archetype(
            "/table".into(),
            &TableBlueprintArchetype::update_fields().with_layout(layout),
        );
    }

    /// Iterates over all columns for the given layout.
    pub fn iter_columns(&self, layout: TableLayoutKind) -> impl Iterator<Item = &TableColumn<'a>> {
        match layout {
            TableLayoutKind::Table => itertools::Either::Left(self.table_layout.columns.iter()),

            TableLayoutKind::Cards => {
                itertools::Either::Right(self.card_layout.iter().flat_map(CardLayout::fields))
            }
        }
    }

    pub fn iter_visible_columns(
        &self,
        layout: TableLayoutKind,
    ) -> impl Iterator<Item = &TableColumn<'a>> {
        self.iter_columns(layout)
            .filter(move |c| c.is_visible(layout))
    }

    pub fn column_by_physical_name(
        &self,
        layout: TableLayoutKind,
        physical_name: &ColumnName,
    ) -> Option<&TableColumn<'a>> {
        self.iter_columns(layout)
            .find(|column| column.physical_name() == physical_name)
    }
}
