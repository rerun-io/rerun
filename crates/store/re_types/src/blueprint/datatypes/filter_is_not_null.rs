// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/filter_is_not_null.fbs".

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

use ::re_types_core::try_serialize_field;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch as _, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Datatype**: Configuration for the filter is not null feature of the dataframe view.
///
/// ⚠️ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FilterIsNotNull {
    /// Whether the filter by event feature is active.
    pub active: crate::datatypes::Bool,

    /// The column used when the filter by event feature is used.
    pub column: crate::blueprint::datatypes::ComponentColumnSelector,
}

::re_types_core::macros::impl_into_cow!(FilterIsNotNull);

impl ::re_types_core::Loggable for FilterIsNotNull {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow::datatypes::*;
        DataType::Struct(Fields::from(vec![
            Field::new("active", <crate::datatypes::Bool>::arrow_datatype(), false),
            Field::new(
                "column",
                <crate::blueprint::datatypes::ComponentColumnSelector>::arrow_datatype(),
                false,
            ),
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
        use ::re_types_core::{arrow_helpers::as_array_ref, Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};
        Ok({
            let fields = Fields::from(vec![
                Field::new("active", <crate::datatypes::Bool>::arrow_datatype(), false),
                Field::new(
                    "column",
                    <crate::blueprint::datatypes::ComponentColumnSelector>::arrow_datatype(),
                    false,
                ),
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
                        let (somes, active): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.active.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let active_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        as_array_ref(BooleanArray::new(
                            BooleanBuffer::from(
                                active
                                    .into_iter()
                                    .map(|datum| datum.map(|datum| datum.0).unwrap_or_default())
                                    .collect::<Vec<_>>(),
                            ),
                            active_validity,
                        ))
                    },
                    {
                        let (somes, column): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.column.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let column_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            _ = column_validity;
                            crate::blueprint::datatypes::ComponentColumnSelector::to_arrow_opt(
                                column,
                            )?
                        }
                    },
                ],
                validity,
            ))
        })
    }

    fn from_arrow_opt(
        arrow_data: &dyn arrow::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{arrow_zip_validity::ZipValidity, Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow::array::StructArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.blueprint.datatypes.FilterIsNotNull")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let (arrow_data_fields, arrow_data_arrays) =
                    (arrow_data.fields(), arrow_data.columns());
                let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data_fields
                    .iter()
                    .map(|field| field.name().as_str())
                    .zip(arrow_data_arrays)
                    .collect();
                let active = {
                    if !arrays_by_name.contains_key("active") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "active",
                        ))
                        .with_context("rerun.blueprint.datatypes.FilterIsNotNull");
                    }
                    let arrow_data = &**arrays_by_name["active"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<BooleanArray>()
                        .ok_or_else(|| {
                            let expected = DataType::Boolean;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.blueprint.datatypes.FilterIsNotNull#active")?
                        .into_iter()
                        .map(|res_or_opt| res_or_opt.map(crate::datatypes::Bool))
                };
                let column = {
                    if !arrays_by_name.contains_key("column") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "column",
                        ))
                        .with_context("rerun.blueprint.datatypes.FilterIsNotNull");
                    }
                    let arrow_data = &**arrays_by_name["column"];
                    crate::blueprint::datatypes::ComponentColumnSelector::from_arrow_opt(arrow_data)
                        .with_context("rerun.blueprint.datatypes.FilterIsNotNull#column")?
                        .into_iter()
                };
                ZipValidity::new_with_validity(
                    ::itertools::izip!(active, column),
                    arrow_data.nulls(),
                )
                .map(|opt| {
                    opt.map(|(active, column)| {
                        Ok(Self {
                            active: active
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.blueprint.datatypes.FilterIsNotNull#active")?,
                            column: column
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.blueprint.datatypes.FilterIsNotNull#column")?,
                        })
                    })
                    .transpose()
                })
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.blueprint.datatypes.FilterIsNotNull")?
            }
        })
    }
}

impl ::re_byte_size::SizeBytes for FilterIsNotNull {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.active.heap_size_bytes() + self.column.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::Bool>::is_pod()
            && <crate::blueprint::datatypes::ComponentColumnSelector>::is_pod()
    }
}
