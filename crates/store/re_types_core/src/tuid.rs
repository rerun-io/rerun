use std::sync::Arc;

use arrow::array::{ArrayRef, AsArray as _, FixedSizeBinaryArray, FixedSizeBinaryBuilder};
use arrow::datatypes::DataType;
use re_tuid::Tuid;

use crate::{DeserializationError, Loggable};

// ---

#[expect(clippy::cast_possible_wrap)]
const BYTE_WIDTH: i32 = std::mem::size_of::<Tuid>() as i32;

pub fn tuids_to_arrow(tuids: &[Tuid]) -> FixedSizeBinaryArray {
    #[expect(clippy::unwrap_used)] // Can't fail
    <Tuid as Loggable>::to_arrow(tuids.iter())
        .unwrap()
        .as_fixed_size_binary()
        .clone()
}

impl Loggable for Tuid {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        DataType::FixedSizeBinary(BYTE_WIDTH)
    }

    fn to_arrow_opt<'a>(
        _data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> crate::SerializationResult<ArrayRef>
    where
        Self: 'a,
    {
        Err(crate::SerializationError::not_implemented(
            Self::ARROW_EXTENSION_NAME,
            "TUIDs are never nullable, use `to_arrow()` instead",
        ))
    }

    #[inline]
    fn to_arrow<'a>(
        iter: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
    ) -> crate::SerializationResult<ArrayRef>
    where
        Self: 'a,
    {
        let iter = iter.into_iter();

        let mut builder = FixedSizeBinaryBuilder::with_capacity(iter.size_hint().0, BYTE_WIDTH);
        for tuid in iter {
            #[expect(clippy::unwrap_used)] // Can't fail because `BYTE_WIDTH` is correct.
            builder.append_value(tuid.into().as_bytes()).unwrap();
        }

        Ok(Arc::new(builder.finish()))
    }

    fn from_arrow(array: &dyn ::arrow::array::Array) -> crate::DeserializationResult<Vec<Self>> {
        let Some(array) = array.as_fixed_size_binary_opt() else {
            return Err(DeserializationError::datatype_mismatch(
                Self::arrow_datatype(),
                array.data_type().clone(),
            ));
        };

        // NOTE: We don't even look at the validity, our datatype says we don't care.

        let uuids: &[Self] = Self::slice_from_bytes(array.value_data()).map_err(|err| {
            DeserializationError::ValidationError(format!("Bad length of Tuid array: {err}"))
        })?;

        Ok(uuids.to_vec())
    }
}

/// Implements [`crate::Component`] for any given type that is a simple wrapper
/// (newtype) around a [`Tuid`].
///
/// Usage:
/// ```ignore
/// re_types_core::delegate_arrow_tuid!(RowId);
/// ```
#[macro_export]
macro_rules! delegate_arrow_tuid {
    ($typ:ident as $fqname:expr) => {
        $crate::macros::impl_into_cow!($typ);

        impl $typ {
            #[inline]
            pub fn partial_descriptor() -> $crate::ComponentDescriptor {
                $crate::ComponentDescriptor::partial($fqname)
            }
        }

        impl $crate::Loggable for $typ {
            #[inline]
            fn arrow_datatype() -> ::arrow::datatypes::DataType {
                $crate::external::re_tuid::Tuid::arrow_datatype()
            }

            #[inline]
            fn to_arrow_opt<'a>(
                _values: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
            ) -> $crate::SerializationResult<arrow::array::ArrayRef>
            where
                Self: 'a,
            {
                Err($crate::SerializationError::not_implemented(
                    <Self as $crate::Component>::name(),
                    "TUIDs are never nullable, use `to_arrow()` instead",
                ))
            }

            #[inline]
            fn to_arrow<'a>(
                values: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
            ) -> $crate::SerializationResult<arrow::array::ArrayRef> {
                let values = values.into_iter().map(|value| {
                    let value: ::std::borrow::Cow<'a, Self> = value.into();
                    value.into_owned()
                });
                <$crate::external::re_tuid::Tuid as $crate::Loggable>::to_arrow(
                    values.into_iter().map(|$typ(tuid)| tuid),
                )
            }

            #[inline]
            fn from_arrow(
                array: &dyn arrow::array::Array,
            ) -> $crate::DeserializationResult<Vec<Self>> {
                Ok(
                    <$crate::external::re_tuid::Tuid as $crate::Loggable>::from_arrow(array)?
                        .into_iter()
                        .map(|tuid| Self(tuid))
                        .collect(),
                )
            }
        }

        impl $crate::Component for $typ {
            #[inline]
            fn name() -> $crate::ComponentType {
                $fqname.into()
            }
        }
    };
}
