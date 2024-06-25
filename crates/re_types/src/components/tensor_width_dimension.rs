// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/components/tensor_dimension_selection.fbs".

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

/// **Component**: Specifies which dimension to use for width.
#[derive(Clone, Debug, Hash, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct TensorWidthDimension(pub crate::datatypes::TensorDimensionSelection);

impl ::re_types_core::SizeBytes for TensorWidthDimension {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::TensorDimensionSelection>::is_pod()
    }
}

impl<T: Into<crate::datatypes::TensorDimensionSelection>> From<T> for TensorWidthDimension {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::TensorDimensionSelection> for TensorWidthDimension {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::TensorDimensionSelection {
        &self.0
    }
}

impl std::ops::Deref for TensorWidthDimension {
    type Target = crate::datatypes::TensorDimensionSelection;

    #[inline]
    fn deref(&self) -> &crate::datatypes::TensorDimensionSelection {
        &self.0
    }
}

impl std::ops::DerefMut for TensorWidthDimension {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::TensorDimensionSelection {
        &mut self.0
    }
}

::re_types_core::macros::impl_into_cow!(TensorWidthDimension);

impl ::re_types_core::Loggable for TensorWidthDimension {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.TensorWidthDimension".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Struct(std::sync::Arc::new(vec![
            Field::new("dimension", DataType::UInt32, false),
            Field::new("invert", DataType::Boolean, false),
        ]))
    }

    #[allow(clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data0): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum.map(|datum| datum.into_owned().0);
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                _ = data0_bitmap;
                crate::datatypes::TensorDimensionSelection::to_arrow_opt(data0)?
            }
        })
    }

    #[allow(clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok(
            crate::datatypes::TensorDimensionSelection::from_arrow_opt(arrow_data)
                .with_context("rerun.components.TensorWidthDimension#dimension")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .map(|res| res.map(|v| Some(Self(v))))
                .collect::<DeserializationResult<Vec<Option<_>>>>()
                .with_context("rerun.components.TensorWidthDimension#dimension")
                .with_context("rerun.components.TensorWidthDimension")?,
        )
    }
}
