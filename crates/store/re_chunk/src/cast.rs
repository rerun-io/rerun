use std::borrow::Cow;

use itertools::Either;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_span::Span;
use re_types_core::ComponentIdentifier;

use crate::ChunkComponentSlicer;

/// Generic component slicer that casts to a primitive type.
///
/// In the happy path (when the array is already the target type), this performs zero-copy slicing.
/// When casting is required, it allocates and owns the casted array.
pub struct CastToPrimitive<P, T>
where
    P: arrow::array::ArrowPrimitiveType<Native = T>,
    T: arrow::datatypes::ArrowNativeType,
{
    _phantom: std::marker::PhantomData<(P, T)>,
}

/// Iterator that owns the array values and component spans.
///
/// This is necessary when we need to cast the array, as the casted array
/// must be owned by the iterator rather than borrowed from the caller.
struct OwnedSliceIterator<'a, T, I>
where
    T: arrow::datatypes::ArrowNativeType,
    I: Iterator<Item = Span<usize>>,
{
    values: Cow<'a, arrow::buffer::ScalarBuffer<T>>,
    component_spans: I,
}

impl<'a, T, I> Iterator for OwnedSliceIterator<'a, T, I>
where
    T: arrow::datatypes::ArrowNativeType + Clone,
    I: Iterator<Item = Span<usize>>,
{
    type Item = Cow<'a, [T]>;

    fn next(&mut self) -> Option<Self::Item> {
        let span = self.component_spans.next()?;
        match &self.values {
            Cow::Borrowed(values) => Some(Cow::Borrowed(&values[span.range()])),
            // TODO(grtlr): This `clone` here makes me sad, but I don't see a way around it.
            Cow::Owned(values) => Some(Cow::Owned(values[span.range()].to_vec())),
        }
    }
}

fn error_on_cast_failure(
    component: ComponentIdentifier,
    target: &arrow::datatypes::DataType,
    actual: &arrow::datatypes::DataType,
    error: &arrow::error::ArrowError,
) {
    if cfg!(debug_assertions) {
        panic!(
            "[DEBUG ASSERT] cast from {actual:?} to {target:?} failed for {component}: {error}. Data discarded"
        );
    } else {
        re_log::error_once!(
            "cast from {actual:?} to {target:?} failed for {component}: {error}. Data discarded"
        );
    }
}

pub fn error_on_downcast_failure(
    component: ComponentIdentifier,
    target: &str,
    actual: &arrow::datatypes::DataType,
) {
    if cfg!(debug_assertions) {
        panic!(
            "[DEBUG ASSERT] downcast to {target} failed for {component}. Array data type was {actual:?}. Data discarded"
        );
    } else {
        re_log::error_once!(
            "downcast to {target} failed for {component}. Array data type was {actual:?}. Data discarded"
        );
    }
}

impl<P, T> ChunkComponentSlicer for CastToPrimitive<P, T>
where
    P: arrow::array::ArrowPrimitiveType<Native = T>,
    T: arrow::datatypes::ArrowNativeType + Clone,
{
    type Item<'a> = Cow<'a, [T]>;

    fn slice<'a>(
        component: ComponentIdentifier,
        array: &'a dyn arrow::array::Array,
        component_spans: impl Iterator<Item = Span<usize>> + 'a,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        // We first try to down cast (happy path - zero copy).
        if let Some(values) = array.downcast_array_ref::<arrow::array::PrimitiveArray<P>>() {
            return Either::Right(OwnedSliceIterator {
                values: Cow::Borrowed(values.values()),
                component_spans,
            });
        }

        // Then we try to perform a primitive cast (requires ownership).
        let casted = match arrow::compute::cast(array, &P::DATA_TYPE) {
            Ok(casted) => casted,
            Err(err) => {
                error_on_cast_failure(component, &P::DATA_TYPE, array.data_type(), &err);
                return Either::Left(std::iter::empty());
            }
        };

        let Some(values) = casted.downcast_array_ref::<arrow::array::PrimitiveArray<P>>() else {
            error_on_downcast_failure(component, "ArrowPrimitiveArray<T>", array.data_type());
            return Either::Left(std::iter::empty());
        };

        Either::Right(OwnedSliceIterator {
            values: Cow::Owned(values.values().clone()),
            component_spans,
        })
    }
}
