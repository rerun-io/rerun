// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/components/class_id.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

/// A 16-bit ID representing a type of semantic class.
///
/// Used to look up a [`crate::datatypes::ClassDescription`] within the [`crate::components::AnnotationContext`].
#[derive(Clone, Debug, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct ClassId(pub crate::datatypes::ClassId);

impl<T: Into<crate::datatypes::ClassId>> From<T> for ClassId {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::ClassId> for ClassId {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::ClassId {
        &self.0
    }
}

impl std::ops::Deref for ClassId {
    type Target = crate::datatypes::ClassId;

    #[inline]
    fn deref(&self) -> &crate::datatypes::ClassId {
        &self.0
    }
}

impl<'a> From<ClassId> for ::std::borrow::Cow<'a, ClassId> {
    #[inline]
    fn from(value: ClassId) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a ClassId> for ::std::borrow::Cow<'a, ClassId> {
    #[inline]
    fn from(value: &'a ClassId) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Loggable for ClassId {
    type Name = crate::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.ClassId".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::UInt16
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        re_tracing::profile_function!();
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data0): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum.map(|datum| {
                        let Self(data0) = datum.into_owned();
                        data0
                    });
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            PrimitiveArray::new(
                Self::arrow_datatype(),
                data0
                    .into_iter()
                    .map(|datum| {
                        datum
                            .map(|datum| {
                                let crate::datatypes::ClassId(data0) = datum;
                                data0
                            })
                            .unwrap_or_default()
                    })
                    .collect(),
                data0_bitmap,
            )
            .boxed()
        })
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        re_tracing::profile_function!();
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, buffer::*, datatypes::*};
        Ok(arrow_data
            .as_any()
            .downcast_ref::<UInt16Array>()
            .ok_or_else(|| {
                crate::DeserializationError::datatype_mismatch(
                    DataType::UInt16,
                    arrow_data.data_type().clone(),
                )
            })
            .with_context("rerun.components.ClassId#id")?
            .into_iter()
            .map(|opt| opt.copied())
            .map(|res_or_opt| res_or_opt.map(|v| crate::datatypes::ClassId(v)))
            .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
            .map(|res| res.map(|v| Some(Self(v))))
            .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.components.ClassId#id")
            .with_context("rerun.components.ClassId")?)
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn from_arrow(
        arrow_data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        re_tracing::profile_function!();
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, buffer::*, datatypes::*};
        if let Some(validity) = arrow_data.validity() {
            if validity.unset_bits() != 0 {
                return Err(crate::DeserializationError::missing_data());
            }
        }
        Ok(arrow_data
            .as_any()
            .downcast_ref::<UInt16Array>()
            .ok_or_else(|| {
                crate::DeserializationError::datatype_mismatch(
                    DataType::UInt16,
                    arrow_data.data_type().clone(),
                )
            })
            .with_context("rerun.components.ClassId#id")?
            .values()
            .as_slice()
            .iter()
            .copied()
            .map(|v| crate::datatypes::ClassId(v))
            .map(|v| Self(v))
            .collect::<Vec<_>>())
    }
}
