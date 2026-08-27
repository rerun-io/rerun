use arrow::array::{ArrayRef, AsArray as _, FixedSizeBinaryArray};
use re_tuid::Tuid;

use crate::{ArrowDatatype, DeserializationError, FromArrow, ToArrow};

// ---

pub fn tuids_to_arrow(tuids: &[Tuid]) -> FixedSizeBinaryArray {
    #[expect(clippy::unwrap_used)] // Can't fail
    <Tuid as ToArrow>::to_arrow(tuids.iter())
        .unwrap()
        .as_fixed_size_binary()
        .clone()
}

impl ArrowDatatype for Tuid {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        quiver::Column::<Self>::datatype()
    }
}

impl ToArrow for Tuid {
    #[inline]
    fn to_arrow<'a>(
        iter: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
    ) -> crate::SerializationResult<ArrayRef>
    where
        Self: 'a,
    {
        let column = quiver::Column::<Self>::from_values(
            iter.into_iter().map(|tuid| tuid.into().into_owned()),
        );
        Ok(column.into_arrow())
    }
}

impl FromArrow for Tuid {
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

        impl $crate::ArrowDatatype for $typ {
            #[inline]
            fn arrow_datatype() -> ::arrow::datatypes::DataType {
                <$crate::external::re_tuid::Tuid as $crate::ArrowDatatype>::arrow_datatype()
            }
        }

        impl $crate::ToArrow for $typ {
            #[inline]
            fn to_arrow<'a>(
                values: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
            ) -> $crate::SerializationResult<arrow::array::ArrayRef> {
                let values = values.into_iter().map(|value| {
                    let value: ::std::borrow::Cow<'a, Self> = value.into();
                    value.into_owned()
                });
                <$crate::external::re_tuid::Tuid as $crate::ToArrow>::to_arrow(
                    values.into_iter().map(|$typ(tuid)| tuid),
                )
            }
        }

        impl $crate::FromArrow for $typ {
            #[inline]
            fn from_arrow(
                array: &dyn arrow::array::Array,
            ) -> $crate::DeserializationResult<Vec<Self>> {
                Ok(
                    <$crate::external::re_tuid::Tuid as $crate::FromArrow>::from_arrow(array)?
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
