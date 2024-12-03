// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/force_center.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Datatype**: Defines a force that globally centers a graph by moving its center of mass towards a given position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForceCenter {
    /// Whether the force is enabled.
    pub enabled: bool,

    /// The `x` position to pull nodes towards.
    pub x: f64,

    /// The `y` position to pull nodes towards.
    pub y: f64,

    /// The strength of the force.
    pub strength: f64,
}

impl ::re_types_core::SizeBytes for ForceCenter {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.enabled.heap_size_bytes()
            + self.x.heap_size_bytes()
            + self.y.heap_size_bytes()
            + self.strength.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <bool>::is_pod() && <f64>::is_pod() && <f64>::is_pod() && <f64>::is_pod()
    }
}

::re_types_core::macros::impl_into_cow!(ForceCenter);

impl ::re_types_core::Loggable for ForceCenter {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow::datatypes::*;
        DataType::Struct(Fields::from(vec![
            Field::new("enabled", DataType::Boolean, false),
            Field::new("x", DataType::Float64, false),
            Field::new("y", DataType::Float64, false),
            Field::new("strength", DataType::Float64, false),
        ]))
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        #![allow(clippy::wildcard_imports)]
        #![allow(clippy::manual_is_variant_and)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};

        #[allow(unused)]
        fn as_array_ref<T: Array + 'static>(t: T) -> ArrayRef {
            std::sync::Arc::new(t) as ArrayRef
        }
        Ok({
            let fields = Fields::from(vec![
                Field::new("enabled", DataType::Boolean, false),
                Field::new("x", DataType::Float64, false),
                Field::new("y", DataType::Float64, false),
                Field::new("strength", DataType::Float64, false),
            ]);
            let (somes, data): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    (datum.is_some(), datum)
                })
                .unzip();
            let validity: Option<arrow::buffer::NullBuffer> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            as_array_ref(StructArray::new(
                fields,
                vec![
                    {
                        let (somes, enabled): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.enabled.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let enabled_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        as_array_ref(BooleanArray::new(
                            BooleanBuffer::from(
                                enabled
                                    .into_iter()
                                    .map(|v| v.unwrap_or_default())
                                    .collect::<Vec<_>>(),
                            ),
                            enabled_validity,
                        ))
                    },
                    {
                        let (somes, x): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.x.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let x_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        as_array_ref(PrimitiveArray::<Float64Type>::new(
                            ScalarBuffer::from(
                                x.into_iter()
                                    .map(|v| v.unwrap_or_default())
                                    .collect::<Vec<_>>(),
                            ),
                            x_validity,
                        ))
                    },
                    {
                        let (somes, y): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.y.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let y_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        as_array_ref(PrimitiveArray::<Float64Type>::new(
                            ScalarBuffer::from(
                                y.into_iter()
                                    .map(|v| v.unwrap_or_default())
                                    .collect::<Vec<_>>(),
                            ),
                            y_validity,
                        ))
                    },
                    {
                        let (somes, strength): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.strength.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let strength_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        as_array_ref(PrimitiveArray::<Float64Type>::new(
                            ScalarBuffer::from(
                                strength
                                    .into_iter()
                                    .map(|v| v.unwrap_or_default())
                                    .collect::<Vec<_>>(),
                            ),
                            strength_validity,
                        ))
                    },
                ],
                validity,
            ))
        })
    }

    fn from_arrow2_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow::datatypes::*;
        use arrow2::{array::*, buffer::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::StructArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.blueprint.datatypes.ForceCenter")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let (arrow_data_fields, arrow_data_arrays) =
                    (arrow_data.fields(), arrow_data.values());
                let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data_fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .zip(arrow_data_arrays)
                    .collect();
                let enabled = {
                    if !arrays_by_name.contains_key("enabled") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "enabled",
                        ))
                        .with_context("rerun.blueprint.datatypes.ForceCenter");
                    }
                    let arrow_data = &**arrays_by_name["enabled"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<BooleanArray>()
                        .ok_or_else(|| {
                            let expected = DataType::Boolean;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.blueprint.datatypes.ForceCenter#enabled")?
                        .into_iter()
                };
                let x = {
                    if !arrays_by_name.contains_key("x") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "x",
                        ))
                        .with_context("rerun.blueprint.datatypes.ForceCenter");
                    }
                    let arrow_data = &**arrays_by_name["x"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<Float64Array>()
                        .ok_or_else(|| {
                            let expected = DataType::Float64;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.blueprint.datatypes.ForceCenter#x")?
                        .into_iter()
                        .map(|opt| opt.copied())
                };
                let y = {
                    if !arrays_by_name.contains_key("y") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "y",
                        ))
                        .with_context("rerun.blueprint.datatypes.ForceCenter");
                    }
                    let arrow_data = &**arrays_by_name["y"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<Float64Array>()
                        .ok_or_else(|| {
                            let expected = DataType::Float64;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.blueprint.datatypes.ForceCenter#y")?
                        .into_iter()
                        .map(|opt| opt.copied())
                };
                let strength = {
                    if !arrays_by_name.contains_key("strength") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "strength",
                        ))
                        .with_context("rerun.blueprint.datatypes.ForceCenter");
                    }
                    let arrow_data = &**arrays_by_name["strength"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<Float64Array>()
                        .ok_or_else(|| {
                            let expected = DataType::Float64;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.blueprint.datatypes.ForceCenter#strength")?
                        .into_iter()
                        .map(|opt| opt.copied())
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    ::itertools::izip!(enabled, x, y, strength),
                    arrow_data.validity(),
                )
                .map(|opt| {
                    opt.map(|(enabled, x, y, strength)| {
                        Ok(Self {
                            enabled: enabled
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.blueprint.datatypes.ForceCenter#enabled")?,
                            x: x.ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.blueprint.datatypes.ForceCenter#x")?,
                            y: y.ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.blueprint.datatypes.ForceCenter#y")?,
                            strength: strength
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.blueprint.datatypes.ForceCenter#strength")?,
                        })
                    })
                    .transpose()
                })
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.blueprint.datatypes.ForceCenter")?
            }
        })
    }
}
