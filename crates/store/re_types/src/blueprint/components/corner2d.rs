// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/corner_2d.fbs".

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
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: One of four 2D corners, typically used to align objects.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Corner2D {
    /// Left top corner.
    LeftTop = 1,

    /// Right top corner.
    RightTop = 2,

    /// Left bottom corner.
    LeftBottom = 3,

    /// Right bottom corner.
    #[default]
    RightBottom = 4,
}

impl ::re_types_core::Component for Corner2D {
    #[inline]
    fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor::new("rerun.blueprint.components.Corner2D")
    }
}

::re_types_core::macros::impl_into_cow!(Corner2D);

impl ::re_types_core::Loggable for Corner2D {
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
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};

        #[allow(unused)]
        fn as_array_ref<T: Array + 'static>(t: T) -> ArrayRef {
            std::sync::Arc::new(t) as ArrayRef
        }
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
            .with_context("rerun.blueprint.components.Corner2D#enum")?
            .into_iter()
            .map(|opt| opt.copied())
            .map(|typ| match typ {
                Some(1) => Ok(Some(Self::LeftTop)),
                Some(2) => Ok(Some(Self::RightTop)),
                Some(3) => Ok(Some(Self::LeftBottom)),
                Some(4) => Ok(Some(Self::RightBottom)),
                None => Ok(None),
                Some(invalid) => Err(DeserializationError::missing_union_arm(
                    Self::arrow_datatype(),
                    "<invalid>",
                    invalid as _,
                )),
            })
            .collect::<DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.blueprint.components.Corner2D")?)
    }
}

impl std::fmt::Display for Corner2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LeftTop => write!(f, "LeftTop"),
            Self::RightTop => write!(f, "RightTop"),
            Self::LeftBottom => write!(f, "LeftBottom"),
            Self::RightBottom => write!(f, "RightBottom"),
        }
    }
}

impl ::re_types_core::reflection::Enum for Corner2D {
    #[inline]
    fn variants() -> &'static [Self] {
        &[
            Self::LeftTop,
            Self::RightTop,
            Self::LeftBottom,
            Self::RightBottom,
        ]
    }

    #[inline]
    fn docstring_md(self) -> &'static str {
        match self {
            Self::LeftTop => "Left top corner.",
            Self::RightTop => "Right top corner.",
            Self::LeftBottom => "Left bottom corner.",
            Self::RightBottom => "Right bottom corner.",
        }
    }
}

impl ::re_types_core::SizeBytes for Corner2D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}
