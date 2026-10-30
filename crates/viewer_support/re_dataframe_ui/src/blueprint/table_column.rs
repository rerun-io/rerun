use re_log_types::{EntityPath, EntityPathPart};
use re_sdk_types::archetypes::EncodedImage;
use re_sdk_types::blueprint::components::{
    ColumnName, ColumnType, Editable, IncludedContent, TableLayoutKind,
};
use re_sdk_types::components::{Bool, Flag, Scalar, Text, Thumbnail, Url, Visible};
use re_sdk_types::{ComponentDescriptor, ComponentType, blueprint::archetypes};
use re_sorbet::{BatchType, ColumnDescriptorRef};
use re_types_core::{Archetype as _, Component as _, ComponentIdentifier};
use re_viewer_context::{AppBlueprintCtx, BlueprintContext as _};

use crate::datafusion_table_widget::{DataColumns, TableColumnHeuristic};
use crate::display_record_batch::DisplayColumn;

/// Resolved runtime configuration for a table column.
///
/// A column is set up in two steps.
/// [`Self::load`] reads the values from [`archetypes::TableColumn`].
/// A field is `None` if the blueprint has no value for it.
/// `resolve_columns` then matches the column with the table schema, and sets defaults from the
/// schema and the table's heuristics.
/// It only sets a default on a field that is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumn<'a> {
    /// The physical name of the source column where data is read from.
    source: ColumnName,

    /// Sorbet column descriptor if we were able to match the column up with a sorbet column.
    ///
    /// [`resolve_columns`] sets it.
    // TODO(andreas): this is always present after resolve, can we make it non-optional?
    sorbet_column_descriptor: Option<ColumnDescriptorRef<'a>>,

    /// Display name.
    ///
    /// [`Self::load`] sets it from the blueprint, and the table's heuristics can set a default.
    /// If it is `None`, [`Self::display_name`] infers it from the column descriptor.
    name: Option<String>,

    /// Whether the column's values can be edited.
    ///
    /// [`Self::load`] sets it from the blueprint.
    /// If the blueprint has no value, [`resolve_columns`] sets it from the `rerun:is_editable`
    /// metadata of the column's field in the table schema.
    /// After [`resolve_columns`] it is never `None`.
    editable: Option<bool>,

    /// Whether the column is visible in the table layout.
    ///
    /// [`Self::load`] sets it from the blueprint.
    /// [`resolve_columns`] sets a default for columns that are not in the layout's column order.
    /// If it is still `None`, [`Self::is_visible`] uses a default based on the layout kind.
    visible: Option<bool>,

    /// The component identifier the column's values are shown as.
    ///
    /// Only the blueprint sets it.
    /// If it is `None`, the column's own component identifier is used.
    component: Option<ComponentIdentifier>,

    /// The component type the column's values are shown as.
    ///
    /// [`Self::load`] sets it from the blueprint.
    /// If the blueprint has no value, [`resolve_columns`] sets it from the table's heuristics
    /// or infers it from the column descriptor.
    /// If it is still `None`, [`Self::resolved_component_type`] uses the column's own component type.
    column_type: Option<ComponentType>,

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
        column.editable.get_or_insert(data_column.is_editable);
        *column = additional_column_heuristics(layout_kind, &data_column.desc, column.clone())
            .with_inferred_component_type();
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
        column.editable.get_or_insert(data_column.is_editable);
        column = additional_column_heuristics(layout_kind, &data_column.desc, column)
            .with_inferred_component_type();

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

        let column_type = results
            .component_mono::<ColumnType>(
                archetypes::TableColumn::descriptor_column_type().component,
            )
            .map(|column_type| column_type.0)
            .unwrap_or_default();

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
                .map(|editable| *editable.0),
            visible: results
                .component_mono::<Visible>(archetypes::TableColumn::descriptor_visible().component)
                .map(|visible| *visible.0),
            component: column_type
                .component
                .and_then(|component| ComponentIdentifier::try_new(component.as_str()).ok()),
            column_type: column_type
                .component_type_name
                .and_then(|component_type| ComponentType::try_new(component_type.as_str()).ok()),
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
            editable: None,
            visible: None,
            component: None,
            column_type: None,
            preview_views: Vec::new(),
        }
        .with_inferred_component_type()
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

    /// Set the component type unless the stored blueprint configured one.
    pub fn with_default_component_type(mut self, component_type: ComponentType) -> Self {
        self.column_type.get_or_insert(component_type);
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

    /// Set the component type inferred from the column descriptor, unless one is configured already.
    ///
    /// The blob of an [`EncodedImage`] is shown as a [`Thumbnail`].
    /// Columns without a component type are shown as [`Bool`] for booleans,
    /// and as [`Text`] and [`Scalar`] for editable strings and 64-bit floats, which have editors.
    fn with_inferred_component_type(self) -> Self {
        let Some(ColumnDescriptorRef::Component(column)) = self.sorbet_column_descriptor else {
            return self;
        };

        let inferred = if column.component == EncodedImage::descriptor_blob().component {
            Some(Thumbnail::name())
        } else if column.component_type.is_none() {
            untyped_component_type(&column.store_datatype, self.editable())
        } else {
            None
        };

        match inferred {
            Some(component_type) => self.with_default_component_type(component_type),
            None => self,
        }
    }

    /// Resolves the component descriptor the values of this column are shown with.
    ///
    /// This is the descriptor of the column itself, with the component identifier and component type
    /// from the blueprint or the table's heuristics.
    ///
    /// Returns `None` for columns that are not component columns.
    pub fn resolved_component_descriptor(&self) -> Option<ComponentDescriptor> {
        let Some(ColumnDescriptorRef::Component(column)) = self.sorbet_column_descriptor else {
            return None;
        };

        let mut descriptor = column.component_descriptor();
        if let Some(component) = self.component {
            descriptor.component = component;
        }
        descriptor.component_type = self.resolved_component_type();
        Some(descriptor)
    }

    /// Resolves the component descriptor the value at `row` of `column` is shown with.
    ///
    /// This is [`Self::resolved_component_descriptor`], except that a string value which is a
    /// Rerun URI is shown as a [`Url`] when the column has no component type.
    pub fn value_resolved_component_descriptor(
        &self,
        column: &DisplayColumn,
        row: usize,
    ) -> Option<ComponentDescriptor> {
        let descriptor = self.resolved_component_descriptor()?;
        if descriptor.component_type.is_none()
            && let DisplayColumn::Component(column) = column
            && column.string_value_at(row).is_some_and(|value| {
                value.contains("://") && value.parse::<re_uri::RedapUri>().is_ok()
            })
        {
            return Some(descriptor.with_component_type(Url::name()));
        }
        Some(descriptor)
    }

    /// The component type the values of this column are shown as.
    ///
    /// This is the component type from the blueprint or the table's heuristics,
    /// or else the component type of the column itself.
    pub fn resolved_component_type(&self) -> Option<ComponentType> {
        self.column_type.or(match self.sorbet_column_descriptor {
            Some(ColumnDescriptorRef::Component(column)) => column.component_type,
            _ => None,
        })
    }

    /// Whether the column is shown as a [`Flag`].
    pub fn is_flag(&self) -> bool {
        self.resolved_component_type() == Some(Flag::name())
    }

    /// Whether this column shows previews, configured by [`archetypes::TableColumnPreview`].
    pub fn is_preview(&self) -> bool {
        !self.preview_views.is_empty()
    }

    /// Whether editing is requested for this column.
    ///
    /// This is the blueprint's value if it has one, else the `rerun:is_editable` metadata of the
    /// column's field in the table schema.
    /// A `false` in the blueprint disables editing even if the field has `rerun:is_editable` metadata.
    ///
    /// It may not be actually editable depending on the context and various restrictions we have.
    pub fn editable(&self) -> bool {
        self.editable.unwrap_or(false)
    }

    /// Blueprint paths at which the preview views for this column are configured.
    pub fn preview_views_blueprint_paths(&self) -> &[EntityPath] {
        &self.preview_views
    }
}

/// The component type for a column without one, from its datatype or the datatype of its list items.
///
/// `Utf8` and `Float64` only get a component type when the column is editable.
fn untyped_component_type(
    datatype: &arrow::datatypes::DataType,
    editable: bool,
) -> Option<ComponentType> {
    use arrow::datatypes::DataType;

    let datatype = match datatype {
        DataType::List(field) | DataType::LargeList(field) => field.data_type(),
        datatype => datatype,
    };
    match datatype {
        DataType::Boolean => Some(Bool::name()),
        DataType::Utf8 if editable => Some(Text::name()),
        DataType::Float64 if editable => Some(Scalar::name()),
        _ => None,
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

    /// A string value of a column without a component type is shown as a `Url` when it is a
    /// Rerun URI. Other strings stay untyped, and a configured component type is kept.
    #[test]
    fn rerun_uri_values_are_shown_as_urls() {
        use std::sync::Arc;

        use crate::display_record_batch::DisplayRecordBatch;

        let field = arrow::datatypes::Field::new("link", arrow::datatypes::DataType::Utf8, false);
        let descriptor =
            re_sorbet::ComponentColumnDescriptor::from_arrow_field(None, &field).unwrap();
        let values: arrow::array::ArrayRef = Arc::new(arrow::array::StringArray::from(vec![
            "rerun+http://localhost:1234/entry/00000000000000000000000000000001",
            "https://example.com",
            "plain text",
        ]));
        let batch = DisplayRecordBatch::try_new(std::iter::once((
            ColumnDescriptorRef::Component(&descriptor),
            values,
        )))
        .unwrap();
        let display_column = &batch.columns()[0];

        let component_type = |column: &TableColumn<'_>, row| {
            let descriptor = column.value_resolved_component_descriptor(display_column, row)?;
            descriptor.component_type
        };

        let column = TableColumn::new_without_blueprint(
            "link".into(),
            ColumnDescriptorRef::Component(&descriptor),
        );
        assert_eq!(component_type(&column, 0), Some(Url::name()));
        assert_eq!(component_type(&column, 1), None);
        assert_eq!(component_type(&column, 2), None);

        let text_column = column.with_default_component_type(Text::name());
        assert_eq!(component_type(&text_column, 0), Some(Text::name()));
    }
}
