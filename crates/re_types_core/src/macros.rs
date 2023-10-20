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
