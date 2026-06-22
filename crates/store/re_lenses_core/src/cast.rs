//! Opt-in casting of derive lens output columns to match their target component.

use std::sync::{Arc, OnceLock};

use arrow::array::{Array as _, ListArray};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Field};

use re_chunk::EntityPath;
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::reflection::Reflection;

use crate::ast::CastTo;
use crate::error::LensRuntimeError;

/// Cached component reflection, used to resolve [`CastTo::Auto`] targets.
fn reflection() -> Option<&'static Reflection> {
    static REFLECTION: OnceLock<Option<Reflection>> = OnceLock::new();
    REFLECTION
        .get_or_init(|| re_sdk_types::reflection::generate_reflection().ok())
        .as_ref()
}

/// The canonical Arrow element datatype of the component named by `descr`, if known.
fn canonical_datatype(descr: &ComponentDescriptor) -> Option<DataType> {
    let component_type = descr.component_type?;
    reflection()?
        .components
        .get(&component_type)
        .map(|reflection| reflection.datatype.clone())
}

/// Casts the element type of a produced output column according to `cast_to`.
///
/// Returns the list unchanged for a `None` cast or when it already has the
/// target type. Errors (rather than emitting a wrongly-typed column) when the
/// requested cast is not supported by Arrow.
pub(crate) fn apply_output_cast(
    cast_to: Option<&CastTo>,
    descr: &ComponentDescriptor,
    target_entity: &EntityPath,
    list: ListArray,
) -> Result<ListArray, LensRuntimeError> {
    let target = match cast_to {
        None => return Ok(list),
        Some(CastTo::Type(datatype)) => datatype.clone(),
        Some(CastTo::Auto) => {
            canonical_datatype(descr).ok_or_else(|| LensRuntimeError::UnknownComponentType {
                target_entity: target_entity.clone(),
                component: descr.component,
            })?
        }
    };

    let values = list.values();
    if values.data_type() == &target {
        return Ok(list);
    }

    let new_values =
        cast(values, &target).map_err(|source| LensRuntimeError::ComponentCastFailed {
            target_entity: target_entity.clone(),
            component: descr.component,
            source,
        })?;

    // Rebuild the list around the cast values, preserving offsets and validity.
    let new_field = Arc::new(Field::new_list_field(new_values.data_type().clone(), true));
    Ok(ListArray::new(
        new_field,
        list.offsets().clone(),
        new_values,
        list.nulls().cloned(),
    ))
}
