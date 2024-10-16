// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/tensor_dimension_index_slider.fbs".

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

/// **Component**: Show a slider for the index of some dimension of a slider.
#[derive(Clone, Debug, Hash, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct TensorDimensionIndexSlider(pub crate::blueprint::datatypes::TensorDimensionIndexSlider);

impl ::re_types_core::SizeBytes for TensorDimensionIndexSlider {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::blueprint::datatypes::TensorDimensionIndexSlider>::is_pod()
    }
}

impl<T: Into<crate::blueprint::datatypes::TensorDimensionIndexSlider>> From<T>
    for TensorDimensionIndexSlider
{
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::blueprint::datatypes::TensorDimensionIndexSlider>
    for TensorDimensionIndexSlider
{
    #[inline]
    fn borrow(&self) -> &crate::blueprint::datatypes::TensorDimensionIndexSlider {
        &self.0
    }
}

impl std::ops::Deref for TensorDimensionIndexSlider {
    type Target = crate::blueprint::datatypes::TensorDimensionIndexSlider;

    #[inline]
    fn deref(&self) -> &crate::blueprint::datatypes::TensorDimensionIndexSlider {
        &self.0
    }
}

impl std::ops::DerefMut for TensorDimensionIndexSlider {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::blueprint::datatypes::TensorDimensionIndexSlider {
        &mut self.0
    }
}

::re_types_core::macros::impl_into_cow!(TensorDimensionIndexSlider);

impl ::re_types_core::Loggable for TensorDimensionIndexSlider {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.blueprint.components.TensorDimensionIndexSlider".into()
    }

    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        crate::blueprint::datatypes::TensorDimensionIndexSlider::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        crate::blueprint::datatypes::TensorDimensionIndexSlider::to_arrow_opt(data.into_iter().map(
            |datum| {
                datum.map(|datum| match datum.into() {
                    ::std::borrow::Cow::Borrowed(datum) => ::std::borrow::Cow::Borrowed(&datum.0),
                    ::std::borrow::Cow::Owned(datum) => ::std::borrow::Cow::Owned(datum.0),
                })
            },
        ))
    }

    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        crate::blueprint::datatypes::TensorDimensionIndexSlider::from_arrow_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }
}
