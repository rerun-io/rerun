// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/force_collide_radius.fbs".

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

/// **Datatype**: Defines a force that resolves collisions between the radii of `GraphNodes`.
#[derive(Clone, Debug, Default, Clone, PartialEq, Eq)]
pub struct ForceCollideRadius {
    /// Whether the force is enabled.
    pub enabled: bool,

    /// The number of iterations to resolve collisions.
    pub iterations: u32,

    /// The strength of the force.
    pub strength: f64,

    /// An additional padding to apply to each node radius.
    pub padding: f64,
}

impl ::re_types_core::SizeBytes for ForceCollideRadius {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.enabled.heap_size_bytes()
            + self.iterations.heap_size_bytes()
            + self.strength.heap_size_bytes()
            + self.padding.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <bool>::is_pod() && <u32>::is_pod() && <f64>::is_pod() && <f64>::is_pod()
    }
}

::re_types_core::macros::impl_into_cow!(ForceCollideRadius);

impl ::re_types_core::Loggable for ForceCollideRadius {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow::datatypes::*;
        DataType::Struct(Fields::from(vec![
            Field::new("enabled", DataType::Boolean, false),
            Field::new("iterations", DataType::UInt32, false),
            Field::new("strength", DataType::Float64, false),
            Field::new("padding", DataType::Float64, false),
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
                Field::new("iterations", DataType::UInt32, false),
                Field::new("strength", DataType::Float64, false),
                Field::new("padding", DataType::Float64, false),
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
                        let (somes, iterations): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.iterations.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let iterations_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        as_array_ref(PrimitiveArray::<UInt32Type>::new(
                            ScalarBuffer::from(
                                iterations
                                    .into_iter()
                                    .map(|v| v.unwrap_or_default())
                                    .collect::<Vec<_>>(),
                            ),
                            iterations_validity,
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
                    {
                        let (somes, padding): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.padding.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let padding_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        as_array_ref(PrimitiveArray::<Float64Type>::new(
                            ScalarBuffer::from(
                                padding
                                    .into_iter()
                                    .map(|v| v.unwrap_or_default())
                                    .collect::<Vec<_>>(),
                            ),
                            padding_validity,
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
                .with_context("rerun.blueprint.datatypes.ForceCollideRadius")?;
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
                        .with_context("rerun.blueprint.datatypes.ForceCollideRadius");
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
                        .with_context("rerun.blueprint.datatypes.ForceCollideRadius#enabled")?
                        .into_iter()
                };
                let iterations = {
                    if !arrays_by_name.contains_key("iterations") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "iterations",
                        ))
                        .with_context("rerun.blueprint.datatypes.ForceCollideRadius");
                    }
                    let arrow_data = &**arrays_by_name["iterations"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<UInt32Array>()
                        .ok_or_else(|| {
                            let expected = DataType::UInt32;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.blueprint.datatypes.ForceCollideRadius#iterations")?
                        .into_iter()
                        .map(|opt| opt.copied())
                };
                let strength = {
                    if !arrays_by_name.contains_key("strength") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "strength",
                        ))
                        .with_context("rerun.blueprint.datatypes.ForceCollideRadius");
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
                        .with_context("rerun.blueprint.datatypes.ForceCollideRadius#strength")?
                        .into_iter()
                        .map(|opt| opt.copied())
                };
                let padding = {
                    if !arrays_by_name.contains_key("padding") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "padding",
                        ))
                        .with_context("rerun.blueprint.datatypes.ForceCollideRadius");
                    }
                    let arrow_data = &**arrays_by_name["padding"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<Float64Array>()
                        .ok_or_else(|| {
                            let expected = DataType::Float64;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.blueprint.datatypes.ForceCollideRadius#padding")?
                        .into_iter()
                        .map(|opt| opt.copied())
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    ::itertools::izip!(enabled, iterations, strength, padding),
                    arrow_data.validity(),
                )
                .map(|opt| {
                    opt.map(|(enabled, iterations, strength, padding)| {
                        Ok(Self {
                            enabled: enabled
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context(
                                    "rerun.blueprint.datatypes.ForceCollideRadius#enabled",
                                )?,
                            iterations: iterations
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context(
                                "rerun.blueprint.datatypes.ForceCollideRadius#iterations",
                            )?,
                            strength: strength
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context(
                                    "rerun.blueprint.datatypes.ForceCollideRadius#strength",
                                )?,
                            padding: padding
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context(
                                    "rerun.blueprint.datatypes.ForceCollideRadius#padding",
                                )?,
                        })
                    })
                    .transpose()
                })
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.blueprint.datatypes.ForceCollideRadius")?
            }
        })
    }
}
