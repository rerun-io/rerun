use arrow::array::{Array as _, ArrayRef, AsArray as _, UInt64Array};
use arrow::compute::{CastOptions, cast_with_options};
use arrow::datatypes::{DataType, UInt64Type};
use serde_json::Value;

use re_sdk_types::components::Color;
use re_sdk_types::datatypes::Uuid;
use re_sdk_types::reflection::{ComponentReflection, ComponentReflectionMap};
use re_sdk_types::{
    ChunkId, Component as _, ComponentDescriptor, ComponentType, DeserializationError,
    FromArrow as _, RowId, ToArrow as _,
};
use re_tuid::Tuid;

use crate::from_json::ValueFromJsonError;

/// The encoding of every component that holds a [`Uuid`], as reflection names it.
///
/// Reflection is the only thing that says a column holds UUIDs: unlike row ids, these columns
/// carry no `ARROW:extension:name`, so a reader without it sees 16 opaque bytes.
const UUID_ENCODING: &str = "rerun.encodings.Uuid";

/// Whether `component_type` is one of the two components that hold a [`Tuid`]. Neither has a
/// `.def.rs`, so neither is in the reflection.
fn is_tuid(component_type: ComponentType) -> bool {
    component_type == RowId::name() || component_type == ChunkId::name()
}

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
    // Only Rerun's own components: their type is what says what they mean, where a custom
    // component's datatype alone does not.
    let component_type = descriptor.component_type?;

    // A row or chunk id as its canonical 32-hex-digit string, rather than 16 raw bytes.
    if is_tuid(component_type) {
        return Some(Tuid::from_arrow(batch.as_ref()).map(|tuids| {
            tuids
                .into_iter()
                .map(|tuid| Value::String(tuid.to_string()))
                .collect()
        }));
    }

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

    // A blueprint id (container, maximized view, visualizer instruction) as a hyphenated UUID
    // string, rather than 16 raw bytes.
    if reflection.encoding == Some(UUID_ENCODING) {
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

/// The inverse of [`special_json_from_batch`]: `instances` in their special JSON form, as the
/// component's Arrow array.
///
/// `None` means the component has no special form, and is decoded generically.
pub fn special_batch_from_json(
    descriptor: &ComponentDescriptor,
    instances: &[Value],
    components: &ComponentReflectionMap,
) -> Option<Result<ArrayRef, ValueFromJsonError>> {
    let component_type = descriptor.component_type?;

    if is_tuid(component_type) {
        return Some(tuids_from_json(component_type, instances));
    }

    let reflection = components.get(&component_type)?;

    if component_type == Color::name() {
        return Some(colors_from_json(component_type, instances));
    }

    if reflection.encoding == Some(UUID_ENCODING) {
        return Some(uuids_from_json(component_type, instances));
    }

    reflection
        .is_enum()
        .then(|| enum_from_json(reflection, instances))
}

fn colors_from_json(
    component_type: ComponentType,
    instances: &[Value],
) -> Result<ArrayRef, ValueFromJsonError> {
    let colors = instances
        .iter()
        .map(|instance| {
            instance
                .as_str()
                .and_then(Color::from_hex)
                .ok_or_else(|| ValueFromJsonError::expected("a color like \"#ff0010\"", instance))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Color::to_arrow(colors).map_err(|err| ValueFromJsonError::ToArrow {
        component_type,
        err,
    })
}

fn uuids_from_json(
    component_type: ComponentType,
    instances: &[Value],
) -> Result<ArrayRef, ValueFromJsonError> {
    let uuids = instances
        .iter()
        .map(|instance| {
            instance
                .as_str()
                .and_then(|uuid| uuid::Uuid::parse_str(uuid).ok())
                .map(Uuid::from)
                .ok_or_else(|| ValueFromJsonError::expected("a UUID string", instance))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Uuid::to_arrow(uuids).map_err(|err| ValueFromJsonError::ToArrow {
        component_type,
        err,
    })
}

fn tuids_from_json(
    component_type: ComponentType,
    instances: &[Value],
) -> Result<ArrayRef, ValueFromJsonError> {
    let tuids = instances
        .iter()
        .map(|instance| {
            instance
                .as_str()
                .and_then(|tuid| tuid.parse::<Tuid>().ok())
                .ok_or_else(|| {
                    ValueFromJsonError::expected(
                        "a TUID string like \"182342300C5F8C327a7b4a6e5a379ac4\"",
                        instance,
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Tuid::to_arrow(tuids).map_err(|err| ValueFromJsonError::ToArrow {
        component_type,
        err,
    })
}

/// Accepts a variant name, or the integer a variant is stored as.
fn enum_from_json(
    reflection: &ComponentReflection,
    instances: &[Value],
) -> Result<ArrayRef, ValueFromJsonError> {
    const EXPECTED: &str = "a variant name or a non-negative integer";

    let integers = instances
        .iter()
        .map(|instance| match instance {
            Value::Null => Ok(None),
            Value::String(name) => reflection
                .enum_variant_value(name)
                .map(Some)
                .ok_or_else(|| ValueFromJsonError::UnknownEnumVariant {
                    name: name.clone(),
                    expected: reflection
                        .enum_variants
                        .unwrap_or_default()
                        .iter()
                        .map(|variant| variant.name)
                        .collect::<Vec<_>>()
                        .join(", "),
                }),
            Value::Number(number) => number
                .as_u64()
                .map(Some)
                .ok_or_else(|| ValueFromJsonError::expected(EXPECTED, instance)),
            Value::Bool(_) | Value::Array(_) | Value::Object(_) => {
                Err(ValueFromJsonError::expected(EXPECTED, instance))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let integers: ArrayRef = std::sync::Arc::new(UInt64Array::from(integers));

    // Not `safe`: an integer too wide for the enum is an error, not a silent null.
    let options = CastOptions {
        safe: false,
        ..Default::default()
    };
    cast_with_options(&integers, &reflection.datatype, &options).map_err(|err| {
        ValueFromJsonError::Decode {
            datatype: reflection.datatype.clone(),
            err,
        }
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
    fn uuid_components_are_recognized_by_their_encoding() {
        let encoding = components().get(&RootContainer::name()).unwrap().encoding;
        assert_eq!(encoding, Some(UUID_ENCODING));
    }

    #[test]
    fn tuids_become_strings() {
        let tuid: Tuid = "182342300C5F8C327a7b4a6e5a379ac4".parse().unwrap();
        let descriptor = ComponentDescriptor::partial("row_id").with_component_type(RowId::name());
        let batch = <Tuid as re_sdk_types::ToArrow>::to_arrow([tuid]).unwrap();

        let json = special_json_from_batch(&descriptor, &batch, components())
            .unwrap()
            .unwrap();
        assert_eq!(json, vec![Value::from("182342300C5F8C327a7b4a6e5a379ac4")]);

        let read_back = special_batch_from_json(&descriptor, &json, components())
            .unwrap()
            .unwrap();
        assert_eq!(read_back.as_ref(), batch.as_ref());
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
