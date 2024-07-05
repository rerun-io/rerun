// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/visual_bounds2d.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: Visual bounds in 2D space used for `Spatial2DView`.
#[derive(Clone, Debug, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct VisualBounds2D(
    /// X and y ranges that should be visible.
    pub crate::datatypes::Range2D,
);

impl ::re_types_core::SizeBytes for VisualBounds2D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::Range2D>::is_pod()
    }
}

impl<T: Into<crate::datatypes::Range2D>> From<T> for VisualBounds2D {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Range2D> for VisualBounds2D {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Range2D {
        &self.0
    }
}

impl std::ops::Deref for VisualBounds2D {
    type Target = crate::datatypes::Range2D;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Range2D {
        &self.0
    }
}

impl std::ops::DerefMut for VisualBounds2D {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::Range2D {
        &mut self.0
    }
}

::re_types_core::macros::impl_into_cow!(VisualBounds2D);

impl ::re_types_core::Loggable for VisualBounds2D {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.blueprint.components.VisualBounds2D".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Struct(std::sync::Arc::new(vec![
            Field::new(
                "x_range",
                <crate::datatypes::Range1D>::arrow_datatype(),
                false,
            ),
            Field::new(
                "y_range",
                <crate::datatypes::Range1D>::arrow_datatype(),
                false,
            ),
        ]))
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        crate::datatypes::Range2D::to_arrow_opt(data.into_iter().map(|datum| {
            datum.map(|datum| match datum.into() {
                ::std::borrow::Cow::Borrowed(datum) => ::std::borrow::Cow::Borrowed(&datum.0),
                ::std::borrow::Cow::Owned(datum) => ::std::borrow::Cow::Owned(datum.0),
            })
        }))
    }

    #[allow(clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        crate::datatypes::Range2D::from_arrow_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(|v| Self(v))).collect())
    }
}
