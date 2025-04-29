use arrow::buffer::OffsetBuffer;
use arrow::datatypes::Field;
use arrow_array::builder::{GenericListBuilder, NullBuilder};
use arrow_array::{Array, ArrayRef, ListArray};
use datafusion::common::{exec_datafusion_err, exec_err};
use datafusion::error::Result as DataFusionResult;
use datafusion::logical_expr::ColumnarValue;
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) fn create_rerun_metadata(
    entity_path: Option<&str>,
    component: &str,
    archetype: Option<&str>,
    archetype_field: Option<&str>,
    kind: &str,
    is_indicator: bool,
) -> HashMap<String, String> {
    let mut metadata: HashMap<String, String> = [
        (
            "rerun.component".to_owned(),
            format!("rerun.components.{component}"),
        ),
        ("rerun.kind".to_owned(), kind.into()),
    ]
    .into_iter()
    .collect();

    if let Some(entity_path) = entity_path {
        metadata.insert("rerun.entity_path".to_owned(), entity_path.to_owned());
    }

    if is_indicator {
        metadata.insert("rerun.is_indicator".to_owned(), "true".to_owned());
    }
    if let Some(archetype) = archetype {
        metadata.insert(
            "rerun.archetype".to_owned(),
            format!("rerun.archetypes.{archetype}"),
        );
    }
    if let Some(archetype_field) = archetype_field {
        metadata.insert("rerun.archetype_field".to_owned(), archetype_field.into());
    }

    metadata
}

pub(crate) fn columnar_value_to_array_of_array<'a>(
    columnar: &'a ColumnarValue,
    name: &str,
) -> DataFusionResult<&'a ListArray> {
    let ColumnarValue::Array(array_ref) = columnar else {
        exec_err!("Unexpected scalar columnar value for {name}")?
    };
    array_ref
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or(exec_datafusion_err!("Incorrect array type for {name}"))
}

pub(crate) fn concatenate_list_of_component_arrays<T>(
    input_arrays: &Vec<Option<ArrayRef>>,
) -> DataFusionResult<ArrayRef>
where
    T: re_types_core::Loggable,
{
    let num_rows = input_arrays.len();

    let mut offsets = Vec::with_capacity(num_rows + 1);
    let mut valid_arrays: Vec<&dyn Array> = Vec::new();
    let mut validity = Vec::with_capacity(num_rows);

    offsets.push(0);
    let mut cumulative_length = 0;

    for opt_array in input_arrays {
        match opt_array {
            Some(array) => {
                // This element is valid
                validity.push(true);
                valid_arrays.push(array);
                cumulative_length += array.len() as i32;
            }
            None => {
                // This element is null
                validity.push(false);
            }
        }
        offsets.push(cumulative_length);
    }

    // Create offset buffer
    let offset_buffer = OffsetBuffer::new(offsets.into());

    // Concatenate all the valid arrays
    let values = if valid_arrays.is_empty() {
        arrow::array::new_empty_array(&T::arrow_datatype())
    } else {
        re_arrow_util::concat_arrays(&valid_arrays)?
    };

    let list_field = Arc::new(Field::new("item", values.data_type().clone(), true));
    Ok(Arc::new(ListArray::try_new(
        list_field,
        offset_buffer,
        values,
        Some(validity.into()),
    )?))
}

pub(crate) fn create_indicator_array(validity: &[bool]) -> ArrayRef {
    let mut indicator_array_builder: GenericListBuilder<i32, NullBuilder> =
        GenericListBuilder::with_capacity(NullBuilder::new(), 0);
    for is_valid in validity {
        indicator_array_builder.append(*is_valid);
    }
    Arc::new(indicator_array_builder.finish()) as ArrayRef
}
