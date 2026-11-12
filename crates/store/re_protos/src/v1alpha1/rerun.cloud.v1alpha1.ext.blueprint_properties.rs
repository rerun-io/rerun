//! Shared logic for the `blueprint` property of a segment in a blueprint dataset.
//!
//! The `blueprint` property holds one component, `name`, the name the blueprint is shown with.

use arrow::array::{ArrayRef, RecordBatch, new_null_array};
use arrow::error::ArrowError;
use re_arrow_util::RecordBatchExt as _;

use super::rerun_cloud_v1alpha1_ext_properties::{
    first_string, list_values_at, property_column, string_list,
};

const BLUEPRINT_PROPERTY: &str = "blueprint";
const BLUEPRINT_NAME_COMPONENT: &str = "name";

/// A blueprint's properties with its name set to `name`.
///
/// `properties` are the blueprint's stored single-row properties, if it has any. Their other
/// columns are kept as they are.
pub fn with_blueprint_name(
    properties: Option<&RecordBatch>,
    name: &str,
) -> Result<RecordBatch, ArrowError> {
    with_name_column(properties, string_list(&[name]))
}

/// A blueprint's stored single-row properties with its name cleared.
///
/// The other columns are kept as they are.
pub fn without_blueprint_name(properties: &RecordBatch) -> Result<RecordBatch, ArrowError> {
    let null_name = new_null_array(string_list(&[]).data_type(), 1);
    with_name_column(Some(properties), null_name)
}

/// `properties` with the name column replaced by the single-row `name`.
fn with_name_column(
    properties: Option<&RecordBatch>,
    name: ArrayRef,
) -> Result<RecordBatch, ArrowError> {
    let name_column = property_column(BLUEPRINT_PROPERTY, BLUEPRINT_NAME_COMPONENT);
    let name_batch = RecordBatch::try_from_iter_with_nullable([(&name_column, name, true)])?;

    match properties {
        Some(properties) => properties
            .clone()
            .filter_columns_by(|field| field.name() != &name_column)?
            .concat_horizontally_with(&name_batch),
        None => Ok(name_batch),
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::StringArray;

    use super::*;

    #[test]
    fn reads_the_name_it_wrote() {
        let batch = with_blueprint_name(None, "overview").unwrap();
        assert_eq!(read_blueprint_name(&batch, 0).as_deref(), Some("overview"));
    }

    /// Renaming a blueprint replaces its name and keeps its other properties.
    #[test]
    fn renaming_keeps_other_properties() {
        let other: ArrayRef = Arc::new(StringArray::from(vec!["x"]));
        let stored = RecordBatch::try_from_iter([("property:unrelated:value", other)]).unwrap();
        let stored = with_blueprint_name(Some(&stored), "first").unwrap();

        let renamed = with_blueprint_name(Some(&stored), "second").unwrap();

        assert_eq!(renamed.num_columns(), 2);
        assert!(renamed.column_by_name("property:unrelated:value").is_some());
        assert_eq!(read_blueprint_name(&renamed, 0).as_deref(), Some("second"));
    }

    /// Clearing a blueprint's name keeps its other properties.
    #[test]
    fn clearing_the_name_keeps_other_properties() {
        let other: ArrayRef = Arc::new(StringArray::from(vec!["x"]));
        let stored = RecordBatch::try_from_iter([("property:unrelated:value", other)]).unwrap();
        let stored = with_blueprint_name(Some(&stored), "first").unwrap();

        let cleared = without_blueprint_name(&stored).unwrap();

        assert_eq!(cleared.num_columns(), 2);
        assert!(cleared.column_by_name("property:unrelated:value").is_some());
        assert_eq!(read_blueprint_name(&cleared, 0), None);
    }

    #[test]
    fn blueprint_without_property_has_no_name() {
        let other: ArrayRef = Arc::new(StringArray::from(vec!["x"]));
        let batch = RecordBatch::try_from_iter([("property:unrelated:value", other)]).unwrap();
        assert_eq!(read_blueprint_name(&batch, 0), None);
    }
}
