//! Shared logic for the `blueprint` property of a segment in a blueprint dataset.
//!
//! The `blueprint` property holds two components:
//! * `name`, the name the blueprint is shown with.
//! * `kind`, the [`BlueprintKind`] of the blueprint.

use arrow::array::{ArrayRef, RecordBatch, new_null_array};
use arrow::error::ArrowError;
use re_arrow_util::RecordBatchExt as _;

use super::rerun_cloud_v1alpha1_ext_properties::{
    first_string, list_values_at, property_column, string_list,
};

const BLUEPRINT_PROPERTY: &str = "blueprint";
const BLUEPRINT_NAME_COMPONENT: &str = "name";
const BLUEPRINT_KIND_COMPONENT: &str = "kind";

/// What a blueprint lays out.
///
/// Stored as the `kind` component, with the string from [`Self::as_str`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlueprintKind {
    /// Lays out the individual recordings of a dataset.
    #[default]
    Recording,

    /// Lays out a table, either a table entry or the segment table of a dataset.
    Table,
}

impl BlueprintKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Table => "table",
        }
    }

    /// The kind with this [`Self::as_str`] string, or `None` for any other string.
    pub fn parse(kind: &str) -> Option<Self> {
        [Self::Recording, Self::Table]
            .into_iter()
            .find(|candidate| candidate.as_str() == kind)
    }
}

/// Returns the blueprint's single-row `properties` with its name set to `name`.
///
/// The other columns stay the same. Without `properties`, the result has only the name column.
pub fn with_blueprint_name(
    properties: Option<&RecordBatch>,
    name: &str,
) -> Result<RecordBatch, ArrowError> {
    with_blueprint_component(properties, BLUEPRINT_NAME_COMPONENT, string_list(&[name]))
}

/// A blueprint's stored single-row properties with its name cleared.
///
/// The other columns are kept as they are.
pub fn without_blueprint_name(properties: &RecordBatch) -> Result<RecordBatch, ArrowError> {
    let null_name = new_null_array(string_list(&[]).data_type(), 1);
    with_blueprint_component(Some(properties), BLUEPRINT_NAME_COMPONENT, null_name)
}

/// Returns the blueprint's single-row `properties` with its kind set to `kind`.
///
/// The other columns stay the same. Without `properties`, the result has only the kind column.
pub fn with_blueprint_kind(
    properties: Option<&RecordBatch>,
    kind: BlueprintKind,
) -> Result<RecordBatch, ArrowError> {
    with_blueprint_component(
        properties,
        BLUEPRINT_KIND_COMPONENT,
        string_list(&[kind.as_str()]),
    )
}

/// Returns `properties` with the `component` column of the `blueprint` property replaced by the
/// single-row `value`.
fn with_blueprint_component(
    properties: Option<&RecordBatch>,
    component: &str,
    value: ArrayRef,
) -> Result<RecordBatch, ArrowError> {
    let component_column = property_column(BLUEPRINT_PROPERTY, component);
    let component_batch =
        RecordBatch::try_from_iter_with_nullable([(&component_column, value, true)])?;

    match properties {
        Some(properties) => properties
            .clone()
            .filter_columns_by(|field| field.name() != &component_column)?
            .concat_horizontally_with(&component_batch),
        None => Ok(component_batch),
    }
}

/// Read a blueprint's name from its `blueprint` property at `row`.
///
/// Returns `None` if the property is absent or the row is null.
pub fn read_blueprint_name(batch: &RecordBatch, row: usize) -> Option<String> {
    let column = property_column(BLUEPRINT_PROPERTY, BLUEPRINT_NAME_COMPONENT);
    let values = list_values_at(batch, row, &column)?;
    first_string(&values)
}

/// Read a blueprint's kind from its `blueprint` property at `row`.
///
/// Returns [`BlueprintKind::Recording`] if the property is absent, the row is null, or the value is
/// not a known kind.
pub fn read_blueprint_kind(batch: &RecordBatch, row: usize) -> BlueprintKind {
    let column = property_column(BLUEPRINT_PROPERTY, BLUEPRINT_KIND_COMPONENT);
    list_values_at(batch, row, &column)
        .and_then(|values| first_string(&values))
        .and_then(|kind| BlueprintKind::parse(&kind))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::StringArray;

    use super::*;

    /// Single-row properties with one column of a property other than `blueprint`.
    fn unrelated_properties() -> RecordBatch {
        let other: ArrayRef = Arc::new(StringArray::from(vec!["x"]));
        RecordBatch::try_from_iter([("property:unrelated:value", other)]).unwrap()
    }

    #[test]
    fn reads_the_name_it_wrote() {
        let batch = with_blueprint_name(None, "overview").unwrap();
        assert_eq!(read_blueprint_name(&batch, 0).as_deref(), Some("overview"));
    }

    /// Renaming a blueprint replaces its name and keeps its other properties.
    #[test]
    fn renaming_keeps_other_properties() {
        let stored = with_blueprint_name(Some(&unrelated_properties()), "first").unwrap();

        let renamed = with_blueprint_name(Some(&stored), "second").unwrap();

        assert_eq!(renamed.num_columns(), 2);
        assert!(renamed.column_by_name("property:unrelated:value").is_some());
        assert_eq!(read_blueprint_name(&renamed, 0).as_deref(), Some("second"));
    }

    /// Clearing a blueprint's name keeps its other properties.
    #[test]
    fn clearing_the_name_keeps_other_properties() {
        let stored = with_blueprint_name(Some(&unrelated_properties()), "first").unwrap();

        let cleared = without_blueprint_name(&stored).unwrap();

        assert_eq!(cleared.num_columns(), 2);
        assert!(cleared.column_by_name("property:unrelated:value").is_some());
        assert_eq!(read_blueprint_name(&cleared, 0), None);
    }

    #[test]
    fn blueprint_without_property_has_no_name() {
        assert_eq!(read_blueprint_name(&unrelated_properties(), 0), None);
    }

    #[test]
    fn reads_the_kind_it_wrote() {
        let table = with_blueprint_kind(None, BlueprintKind::Table).unwrap();
        assert_eq!(read_blueprint_kind(&table, 0), BlueprintKind::Table);

        let recording = with_blueprint_kind(None, BlueprintKind::Recording).unwrap();
        assert_eq!(read_blueprint_kind(&recording, 0), BlueprintKind::Recording);
    }

    /// Setting the kind of a named blueprint keeps its name, and renaming it keeps its kind.
    #[test]
    fn name_and_kind_are_kept_together() {
        let named = with_blueprint_name(None, "overview").unwrap();
        let table = with_blueprint_kind(Some(&named), BlueprintKind::Table).unwrap();
        assert_eq!(read_blueprint_name(&table, 0).as_deref(), Some("overview"));
        assert_eq!(read_blueprint_kind(&table, 0), BlueprintKind::Table);

        let renamed = with_blueprint_name(Some(&table), "detail").unwrap();
        assert_eq!(read_blueprint_name(&renamed, 0).as_deref(), Some("detail"));
        assert_eq!(read_blueprint_kind(&renamed, 0), BlueprintKind::Table);
    }

    /// A blueprint without the `blueprint` property is a recording blueprint.
    #[test]
    fn blueprint_without_property_is_recording() {
        assert_eq!(
            read_blueprint_kind(&unrelated_properties(), 0),
            BlueprintKind::Recording
        );
    }
}
