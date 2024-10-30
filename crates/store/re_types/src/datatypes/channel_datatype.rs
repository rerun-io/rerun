// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/channel_datatype.fbs".

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

/// **Datatype**: The innermost datatype of an image.
///
/// How individual color channel components are encoded.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ChannelDatatype {
    /// 8-bit unsigned integer.
    #[default]
    U8 = 6,

    /// 8-bit signed integer.
    I8 = 7,

    /// 16-bit unsigned integer.
    U16 = 8,

    /// 16-bit signed integer.
    I16 = 9,

    /// 32-bit unsigned integer.
    U32 = 10,

    /// 32-bit signed integer.
    I32 = 11,

    /// 64-bit unsigned integer.
    U64 = 12,

    /// 64-bit signed integer.
    I64 = 13,

    /// 16-bit IEEE-754 floating point, also known as `half`.
    F16 = 33,

    /// 32-bit IEEE-754 floating point, also known as `float` or `single`.
    F32 = 34,

    /// 64-bit IEEE-754 floating point, also known as `double`.
    F64 = 35,
}

::re_types_core::macros::impl_into_cow!(ChannelDatatype);

impl ::re_types_core::Loggable for ChannelDatatype {
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
            .with_context("rerun.datatypes.ChannelDatatype#enum")?
            .into_iter()
            .map(|opt| opt.copied())
            .map(|typ| match typ {
                Some(6) => Ok(Some(Self::U8)),
                Some(7) => Ok(Some(Self::I8)),
                Some(8) => Ok(Some(Self::U16)),
                Some(9) => Ok(Some(Self::I16)),
                Some(10) => Ok(Some(Self::U32)),
                Some(11) => Ok(Some(Self::I32)),
                Some(12) => Ok(Some(Self::U64)),
                Some(13) => Ok(Some(Self::I64)),
                Some(33) => Ok(Some(Self::F16)),
                Some(34) => Ok(Some(Self::F32)),
                Some(35) => Ok(Some(Self::F64)),
                None => Ok(None),
                Some(invalid) => Err(DeserializationError::missing_union_arm(
                    Self::arrow_datatype(),
                    "<invalid>",
                    invalid as _,
                )),
            })
            .collect::<DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.datatypes.ChannelDatatype")?)
    }
}

impl std::fmt::Display for ChannelDatatype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::U8 => write!(f, "U8"),
            Self::I8 => write!(f, "I8"),
            Self::U16 => write!(f, "U16"),
            Self::I16 => write!(f, "I16"),
            Self::U32 => write!(f, "U32"),
            Self::I32 => write!(f, "I32"),
            Self::U64 => write!(f, "U64"),
            Self::I64 => write!(f, "I64"),
            Self::F16 => write!(f, "F16"),
            Self::F32 => write!(f, "F32"),
            Self::F64 => write!(f, "F64"),
        }
    }
}

impl ::re_types_core::reflection::Enum for ChannelDatatype {
    #[inline]
    fn variants() -> &'static [Self] {
        &[
            Self::U8,
            Self::I8,
            Self::U16,
            Self::I16,
            Self::U32,
            Self::I32,
            Self::U64,
            Self::I64,
            Self::F16,
            Self::F32,
            Self::F64,
        ]
    }

    #[inline]
    fn docstring_md(self) -> &'static str {
        match self {
            Self::U8 => "8-bit unsigned integer.",
            Self::I8 => "8-bit signed integer.",
            Self::U16 => "16-bit unsigned integer.",
            Self::I16 => "16-bit signed integer.",
            Self::U32 => "32-bit unsigned integer.",
            Self::I32 => "32-bit signed integer.",
            Self::U64 => "64-bit unsigned integer.",
            Self::I64 => "64-bit signed integer.",
            Self::F16 => "16-bit IEEE-754 floating point, also known as `half`.",
            Self::F32 => "32-bit IEEE-754 floating point, also known as `float` or `single`.",
            Self::F64 => "64-bit IEEE-754 floating point, also known as `double`.",
        }
    }
}

impl ::re_types_core::SizeBytes for ChannelDatatype {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}
