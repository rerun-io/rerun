use arrow::array::{Array as _, ArrayRef, AsArray as _};
use arrow::compute::{CastOptions, cast_with_options};
use arrow::datatypes::{DataType, UInt64Type};
use serde_json::Value;

use re_sdk_types::components::Color;
use re_sdk_types::datatypes::Uuid;
use re_sdk_types::reflection::ComponentReflectionMap;
use re_sdk_types::{
    ArrowDataType as _, Component as _, ComponentDescriptor, DeserializationError, FromArrow as _,
};

/// The JSON form of each instance of `batch`, for the few components whose form differs from
/// what `arrow-json` writes for their datatype.
///
/// Every such exception is listed here; `None` means the component is encoded generically,
/// straight from its Arrow datatype.
pub fn special_json_from_batch(
    descriptor: &ComponentDescriptor,
    batch: &ArrayRef,
    components: &ComponentReflectionMap,
) -> Option<Result<Vec<Value>, DeserializationError>> {
    // Only Rerun's own components: their reflection is what says what they mean, where a custom
    // component's datatype alone does not.
    let component_type = descriptor.component_type?;
    let reflection = components.get(&component_type)?;

    // A color as `"#rrggbb"`, or `"#rrggbbaa"` when not fully opaque, rather than a packed `u32`.
    if component_type == Color::name() {
        return Some(Color::from_arrow(batch.as_ref()).map(|colors| {
            colors
                .into_iter()
                .map(|color| Value::String(color.to_hex()))
                .collect()
        }));
    }

    // A blueprint id (container, maximized view, visualizer instruction, …) as a hyphenated UUID
    // string, rather than 16 raw bytes.
    if reflection.datatype == Uuid::arrow_data_type() {
        return Some(Uuid::from_arrow(batch.as_ref()).map(|uuids| {
            uuids
                .into_iter()
                .map(|uuid| Value::String(uuid.to_string()))
                .collect()
        }));
    }

    // An enum as its variant name, e.g. `"Horizontal"`, rather than the integer it is stored as.
    // A value that names no variant, e.g. one written by a newer Rerun, stays that integer.
    reflection.is_enum().then(|| {
        enum_integers(batch).map(|values| {
            values
                .into_iter()
                .map(|value| match value {
                    Some(integer) => reflection
                        .enum_variant_name(integer)
                        .map_or_else(|| Value::from(integer), Value::from),
                    None => Value::Null,
                })
                .collect()
        })
    })
}

/// The integers an enum batch stores, whichever integer type its enum uses.
fn enum_integers(batch: &ArrayRef) -> Result<Vec<Option<u64>>, DeserializationError> {
    let mismatch =
        || DeserializationError::datatype_mismatch(DataType::UInt64, batch.data_type().clone());
    if !batch.data_type().is_integer() {
        return Err(mismatch());
    }
    // Not `safe`: a value that does not fit (a negative one) is an error, not a silent null.
    let options = CastOptions {
        safe: false,
        ..Default::default()
    };
    let integers =
        cast_with_options(batch, &DataType::UInt64, &options).map_err(|_err| mismatch())?;
    Ok(integers.as_primitive::<UInt64Type>().iter().collect())
}

#[cfg(test)]
mod tests {
    use re_sdk_types::ComponentBatch as _;
    use re_sdk_types::blueprint::components::{ContainerKind, RootContainer};

    use super::*;

    fn components() -> &'static ComponentReflectionMap {
        &re_sdk_types::reflection::reflection().components
    }

    #[test]
    fn only_rerun_components_become_uuids() {
        let batch = re_sdk_types::datatypes::Uuid { bytes: [7; 16] }
            .to_arrow()
            .unwrap();

        let custom = ComponentDescriptor::partial("my_hash").with_component_type("my.Hash".into());
        assert!(special_json_from_batch(&custom, &batch, components()).is_none());

        let root_container = ComponentDescriptor::partial("root_container")
            .with_component_type(RootContainer::name());
        assert_eq!(
            special_json_from_batch(&root_container, &batch, components())
                .unwrap()
                .unwrap(),
            vec![Value::from("07070707-0707-0707-0707-070707070707")]
        );
    }

    #[test]
    fn enums_become_variant_names() {
        let descriptor = ComponentDescriptor::partial("container_kind")
            .with_component_type(ContainerKind::name());
        let batch = ContainerKind::Horizontal.to_arrow().unwrap();

        assert_eq!(
            special_json_from_batch(&descriptor, &batch, components())
                .unwrap()
                .unwrap(),
            vec![Value::from("Horizontal")]
        );

        let unknown: ArrayRef = std::sync::Arc::new(arrow::array::UInt16Array::from(vec![200]));
        assert_eq!(
            special_json_from_batch(&descriptor, &unknown, components())
                .unwrap()
                .unwrap(),
            vec![Value::from(200)]
        );
    }
}
