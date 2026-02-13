use re_byte_size::SizeBytes;

use crate::{Component, ComponentType, DeserializationResult, Loggable, SerializationResult};

/// Implementation helper for [`Component`]s that wrap a single [`Loggable`] datatype.
///
/// This should be almost all components with the exception of enum-components.
pub trait WrapperComponent:
    'static
    + Send
    + Sync
    + Clone
    + Sized
    + SizeBytes
    + From<Self::Datatype>
    + std::ops::Deref<Target = Self::Datatype>
{
    /// The underlying [`Loggable`] datatype for this component.
    type Datatype: Loggable + Sized;

    /// The fully-qualified type of this component, e.g. `rerun.components.Position2D`.
    fn name() -> ComponentType;

    /// Unwraps the component into its underlying datatype.
    fn into_inner(self) -> Self::Datatype;
}

impl<T: WrapperComponent> Component for T {
    fn name() -> ComponentType {
        Self::name()
    }
}

impl<T: WrapperComponent> Loggable for T {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        <Self as WrapperComponent>::Datatype::arrow_datatype()
    }

    // NOTE: Don't inline this, this gets _huge_.
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        <Self as WrapperComponent>::Datatype::to_arrow_opt(data.into_iter().map(|datum| {
            datum.map(|datum| match datum.into() {
                ::std::borrow::Cow::Borrowed(datum) => ::std::borrow::Cow::Borrowed(&**datum),
                ::std::borrow::Cow::Owned(datum) => ::std::borrow::Cow::Owned(
                    <Self as WrapperComponent>::Datatype::from(datum.into_inner()),
                ),
            })
        }))
    }

    // NOTE: Don't inline this, this gets _huge_.
    fn from_arrow_opt(
        arrow_data: &dyn arrow::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        <Self as WrapperComponent>::Datatype::from_arrow_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self::from)).collect())
    }

    #[inline]
    fn from_arrow(arrow_data: &dyn arrow::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        <Self as WrapperComponent>::Datatype::from_arrow(arrow_data)
            .map(|v| v.into_iter().map(Self::from).collect())
    }
}
