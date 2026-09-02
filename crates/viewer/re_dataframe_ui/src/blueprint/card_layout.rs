use re_sdk_types::blueprint::archetypes;
use re_sdk_types::blueprint::components::{ColumnName, TableCellKind, TableLayoutKind};
use re_sorbet::ColumnDescriptorRef;
use re_types_core::Archetype as _;
use re_viewer_context::{AppBlueprintCtx, BlueprintContext as _};

use crate::blueprint::TableColumn;
use crate::blueprint::table_column::resolve_columns;
use crate::datafusion_table_widget::{DataColumns, TableColumnHeuristic};

/// Resolved runtime configuration for displaying table records as cards.
///
/// Every source column is loaded from the table schema.
/// `field_order` makes fields visible by default and orders them first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardLayout<'a> {
    /// The physical name of the source column used for card titles.
    ///
    /// If unset, the first visible string column is used as the title.
    title: Option<ColumnName>,

    /// The physical name of the source column containing the target opened when a card is activated.
    ///
    /// If unset, the first configured preview field is used, then the first inferred URL column.
    link: Option<ColumnName>,

    /// The columns included in each card, in display order.
    fields: Vec<TableColumn<'a>>,
}

impl<'a> CardLayout<'a> {
    /// Returns the physical name of the source column used for card titles.
    pub fn title(&self) -> Option<&ColumnName> {
        self.title.as_ref()
    }

    /// Returns the physical name of the source column opened when a card is activated.
    pub fn link(&self) -> Option<&ColumnName> {
        self.link.as_ref()
    }

    /// Returns the columns included in each card, in display order.
    pub fn fields(&self) -> &[TableColumn<'a>] {
        &self.fields
    }

    /// Loads the card layout from the blueprint and resolves its columns against the table schema.
    ///
    /// Returns `None` only when the blueprint contains no card layout fields at all.
    pub fn load_and_resolve(
        blueprint: &AppBlueprintCtx<'_>,
        data_columns: &'a DataColumns<'a>,
        additional_column_heuristics: &TableColumnHeuristic<'_>,
    ) -> Option<Self> {
        let results = blueprint.latest_at_in_current_blueprint(
            &"/table/layouts/cards".into(),
            archetypes::CardLayout::all_component_identifiers(),
        );
        if results.is_empty() {
            return None;
        }

        let field_order = results
            .component_batch::<ColumnName>(
                archetypes::CardLayout::descriptor_field_order().component,
            )
            .unwrap_or_default();
        let mut fields = field_order
            .into_iter()
            .map(|source| {
                TableColumn::load(source, blueprint, TableLayoutKind::Cards)
                    .with_default_visibility(true) // Mentioned fields are default visible.
            })
            .collect();
        resolve_columns(
            blueprint,
            TableLayoutKind::Cards,
            &mut fields,
            data_columns,
            additional_column_heuristics,
        );

        let link = load_and_resolve_link(data_columns, &results, &fields);
        let title = load_and_resolve_title(data_columns, &results, &fields);

        // Warn if there's more than one flag field since we don't support that yet.
        {
            let mut flags = fields.iter().filter(|field| {
                field.is_visible(TableLayoutKind::Cards)
                    && field.configured_cell_kind() == TableCellKind::Flag
            });
            flags.next();
            if flags.next().is_some() {
                re_log::warn_once!(
                    "Card layout has multiple flag fields; only the first field is shown in the card header"
                );
            }
        }

        Some(Self {
            title,
            link,
            fields,
        })
    }

    /// Returns the first visible field configured for the card's special flag position.
    pub fn flag(&self) -> Option<&TableColumn<'a>> {
        self.fields.iter().find(|field| {
            field.is_visible(TableLayoutKind::Cards)
                && field.configured_cell_kind() == TableCellKind::Flag
        })
    }
}

fn load_and_resolve_title(
    data_columns: &DataColumns<'_>,
    results: &re_entity_db::external::re_query::LatestAtResults,
    fields: &[TableColumn<'_>],
) -> Option<ColumnName> {
    let mut title =
        results.component_mono::<ColumnName>(archetypes::CardLayout::descriptor_title().component);

    // Silently ignore invalid title.
    if title
        .as_ref()
        .is_some_and(|title| data_columns.find_by_physical_name(title).is_none())
    {
        title = None;
    }

    // Try coming up with a title.
    title.or_else(|| {
        fields.iter().find_map(|field| {
            if !field.is_visible(TableLayoutKind::Cards) {
                return None;
            }
            let (_, column) = data_columns.find_by_physical_name(field.physical_name())?;
            matches!(
                &column.desc,
                ColumnDescriptorRef::Component(component)
                    if component.store_datatype == arrow::datatypes::DataType::Utf8
            )
            .then(|| column.physical_name().clone())
        })
    })
}

fn load_and_resolve_link(
    data_columns: &DataColumns<'_>,
    results: &re_entity_db::external::re_query::LatestAtResults,
    fields: &[TableColumn<'_>],
) -> Option<ColumnName> {
    let mut link =
        results.component_mono::<ColumnName>(archetypes::CardLayout::descriptor_link().component);

    // Silently ignore invalid column names on title & link.
    if link
        .as_ref()
        .is_some_and(|link| data_columns.find_by_physical_name(link).is_none())
    {
        link = None;
    }

    link.or_else(|| {
        fields.iter().find_map(|field| {
            (field.configured_cell_kind() == TableCellKind::Preview
                && field.is_visible(TableLayoutKind::Cards))
            .then(|| field.physical_name().clone())
        })
    })
}
