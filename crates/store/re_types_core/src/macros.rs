/// Implements `From<Self>` and `From<'a Self>` for `Cow<Self>`.
#[macro_export]
macro_rules! impl_into_cow {
    ($typ:ident) => {
        impl<'a> From<$typ> for ::std::borrow::Cow<'a, $typ> {
            #[inline]
            fn from(value: $typ) -> Self {
                std::borrow::Cow::Owned(value)
            }
        }

        impl<'a> From<&'a $typ> for ::std::borrow::Cow<'a, $typ> {
            #[inline]
            fn from(value: &'a $typ) -> Self {
                std::borrow::Cow::Borrowed(value)
            }
        }
    };
}

/// Implements [`ToArrow`](crate::ToArrow) in terms of [`ToArrowOpt`](crate::ToArrowOpt).
#[macro_export]
macro_rules! impl_to_arrow_via_to_arrow_opt {
    ($typ:ty) => {
        impl $crate::ToArrow for $typ {
            #[inline]
            fn to_arrow<'a>(
                data: impl IntoIterator<Item = impl Into<::std::borrow::Cow<'a, Self>>>,
            ) -> $crate::SerializationResult<$crate::external::arrow::array::ArrayRef>
            where
                Self: 'a,
            {
                $crate::to_arrow_via_to_arrow_opt(data)
            }
        }
    };
}

/// Implements [`FromArrow`](crate::FromArrow) in terms of [`FromArrowOpt`](crate::FromArrowOpt).
///
/// Deserialization fails if the array contains any nulls.
#[macro_export]
macro_rules! impl_from_arrow_via_from_arrow_opt {
    ($typ:ty) => {
        impl $crate::FromArrow for $typ {
            #[inline]
            fn from_arrow(
                data: &dyn $crate::external::arrow::array::Array,
            ) -> $crate::DeserializationResult<Vec<Self>> {
                $crate::from_arrow_via_from_arrow_opt(data)
            }
        }
    };
}

/// Implements [`FromArrowOpt`](crate::FromArrowOpt) in terms of [`FromArrow`](crate::FromArrow).
///
/// The deserialized values are never `None`.
#[macro_export]
macro_rules! impl_from_arrow_opt_via_from_arrow {
    ($typ:ty) => {
        impl $crate::FromArrowOpt for $typ {
            #[inline]
            fn from_arrow_opt(
                data: &dyn $crate::external::arrow::array::Array,
            ) -> $crate::DeserializationResult<Vec<Option<Self>>> {
                $crate::from_arrow_opt_via_from_arrow(data)
            }
        }
    };
}
