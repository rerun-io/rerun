use std::marker::PhantomData;

use re_byte_size::SizeBytes;

use crate::{DeserializationResult, Loggable, SerializationResult};

/// Implementation helper for [`Component`]s that wrap a single [`Loggable`] datatype.
///
/// This should be almost all components with the exception of enum-components.
///
/// The `NewType` parameter is used to ensure that the `WrapperComponent` can be unique
/// for each component, thus allowing extensions for individual components.
pub struct WrapperComponent<T: Loggable, NewType>(pub T, pub PhantomData<NewType>);

impl<T: Loggable, NewType> Clone for WrapperComponent<T, NewType> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<T: Loggable + Copy, NewType> Copy for WrapperComponent<T, NewType> {}

impl<T: Loggable + Default, NewType> Default for WrapperComponent<T, NewType> {
    #[inline]
    fn default() -> Self {
        Self(T::default(), PhantomData)
    }
}

impl<T: Loggable + std::fmt::Debug, NewType> std::fmt::Debug for WrapperComponent<T, NewType> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: this part sucks
        write!(f, "WrapperComponent({:?})", self.0)
    }
}

impl<T: Loggable + Sized, NewType: Send + Sync + 'static> Loggable
    for WrapperComponent<T, NewType>
{
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        T::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        T::to_arrow_opt(data.into_iter().map(|datum| {
            datum.map(|datum| match datum.into() {
                ::std::borrow::Cow::Borrowed(datum) => ::std::borrow::Cow::Borrowed(&datum.0),
                ::std::borrow::Cow::Owned(datum) => ::std::borrow::Cow::Owned(datum.0),
            })
        }))
    }

    fn from_arrow_opt(
        arrow_data: &dyn arrow::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        T::from_arrow_opt(arrow_data).map(|v| {
            v.into_iter()
                .map(|v| v.map(|v| Self(v, PhantomData)))
                .collect()
        })
    }

    #[inline]
    fn from_arrow(arrow_data: &dyn arrow::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        T::from_arrow(arrow_data).map(|v| v.into_iter().map(|v| Self(v, PhantomData)).collect())
    }
}

impl<T: Loggable, NewType> From<WrapperComponent<T, NewType>>
    for ::std::borrow::Cow<'_, WrapperComponent<T, NewType>>
{
    #[inline]
    fn from(value: WrapperComponent<T, NewType>) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a, T: Loggable, NewType> From<&'a WrapperComponent<T, NewType>>
    for ::std::borrow::Cow<'a, WrapperComponent<T, NewType>>
{
    #[inline]
    fn from(value: &'a WrapperComponent<T, NewType>) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl<T: Loggable, NewType> std::borrow::Borrow<T> for WrapperComponent<T, NewType> {
    #[inline]
    fn borrow(&self) -> &T {
        &self.0
    }
}

impl<T: Loggable, NewType> std::ops::Deref for WrapperComponent<T, NewType> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Loggable, NewType> std::ops::DerefMut for WrapperComponent<T, NewType> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: Loggable, NewType> SizeBytes for WrapperComponent<T, NewType> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        T::is_pod()
    }
}
