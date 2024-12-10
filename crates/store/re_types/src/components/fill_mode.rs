// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/components/fill_mode.fbs".

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
#![allow(non_camel_case_types)]

use ::re_types_core::external::arrow2;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: How a geometric shape is drawn and colored.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FillMode {
    /// Lines are drawn around the parts of the shape which directly correspond to the logged data.
    ///
    /// Examples of what this means:
    ///
    /// * An [`archetypes::Ellipsoids3D`][crate::archetypes::Ellipsoids3D] will draw three axis-aligned ellipses that are cross-sections
    ///   of each ellipsoid, each of which displays two out of three of the sizes of the ellipsoid.
    /// * For [`archetypes::Boxes3D`][crate::archetypes::Boxes3D], it is the edges of the box, identical to [`components::FillMode::DenseWireframe`][crate::components::FillMode::DenseWireframe].
    #[default]
    MajorWireframe = 1,

    /// Many lines are drawn to represent the surface of the shape in a see-through fashion.
    ///
    /// Examples of what this means:
    ///
    /// * An [`archetypes::Ellipsoids3D`][crate::archetypes::Ellipsoids3D] will draw a wireframe triangle mesh that approximates each
    ///   ellipsoid.
    /// * For [`archetypes::Boxes3D`][crate::archetypes::Boxes3D], it is the edges of the box, identical to [`components::FillMode::MajorWireframe`][crate::components::FillMode::MajorWireframe].
    DenseWireframe = 2,

    /// The surface of the shape is filled in with a solid color. No lines are drawn.
    Solid = 3,
}

impl ::re_types_core::Component for FillMode {
    #[inline]
    fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor::new("rerun.components.FillMode")
    }
}

::re_types_core::macros::impl_into_cow!(FillMode);

impl ::re_types_core::Loggable for FillMode {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow::datatypes::*;
        DataType::UInt8
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
            let (somes, data0): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum.map(|datum| *datum as u8);
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_validity: Option<arrow::buffer::NullBuffer> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            as_array_ref(PrimitiveArray::<UInt8Type>::new(
                ScalarBuffer::from(
                    data0
                        .into_iter()
                        .map(|v| v.unwrap_or_default())
                        .collect::<Vec<_>>(),
                ),
                data0_validity,
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
        Ok(arrow_data
            .as_any()
            .downcast_ref::<UInt8Array>()
            .ok_or_else(|| {
                let expected = Self::arrow_datatype();
                let actual = arrow_data.data_type().clone();
                DeserializationError::datatype_mismatch(expected, actual)
            })
            .with_context("rerun.components.FillMode#enum")?
            .into_iter()
            .map(|opt| opt.copied())
            .map(|typ| match typ {
                Some(1) => Ok(Some(Self::MajorWireframe)),
                Some(2) => Ok(Some(Self::DenseWireframe)),
                Some(3) => Ok(Some(Self::Solid)),
                None => Ok(None),
                Some(invalid) => Err(DeserializationError::missing_union_arm(
                    Self::arrow_datatype(),
                    "<invalid>",
                    invalid as _,
                )),
            })
            .collect::<DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.components.FillMode")?)
    }
}

impl std::fmt::Display for FillMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MajorWireframe => write!(f, "MajorWireframe"),
            Self::DenseWireframe => write!(f, "DenseWireframe"),
            Self::Solid => write!(f, "Solid"),
        }
    }
}

impl ::re_types_core::reflection::Enum for FillMode {
    #[inline]
    fn variants() -> &'static [Self] {
        &[Self::MajorWireframe, Self::DenseWireframe, Self::Solid]
    }

    #[inline]
    fn docstring_md(self) -> &'static str {
        match self {
            Self::MajorWireframe => {
                "Lines are drawn around the parts of the shape which directly correspond to the logged data.\n\nExamples of what this means:\n\n* An [`archetypes.Ellipsoids3D`](https://rerun.io/docs/reference/types/archetypes/ellipsoids3d) will draw three axis-aligned ellipses that are cross-sections\n  of each ellipsoid, each of which displays two out of three of the sizes of the ellipsoid.\n* For [`archetypes.Boxes3D`](https://rerun.io/docs/reference/types/archetypes/boxes3d), it is the edges of the box, identical to [`components.FillMode#DenseWireframe`](https://rerun.io/docs/reference/types/components/fill_mode)."
            }
            Self::DenseWireframe => {
                "Many lines are drawn to represent the surface of the shape in a see-through fashion.\n\nExamples of what this means:\n\n* An [`archetypes.Ellipsoids3D`](https://rerun.io/docs/reference/types/archetypes/ellipsoids3d) will draw a wireframe triangle mesh that approximates each\n  ellipsoid.\n* For [`archetypes.Boxes3D`](https://rerun.io/docs/reference/types/archetypes/boxes3d), it is the edges of the box, identical to [`components.FillMode#MajorWireframe`](https://rerun.io/docs/reference/types/components/fill_mode)."
            }
            Self::Solid => {
                "The surface of the shape is filled in with a solid color. No lines are drawn."
            }
        }
    }
}

impl ::re_types_core::SizeBytes for FillMode {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}
