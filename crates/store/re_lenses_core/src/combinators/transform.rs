use arrow::array::{Array, ListArray};

use super::error::Error;

/// A fallible transformation from one Arrow array to another.
///
/// Can be composed using the [`then`](Transform::then) method to create transformation pipelines.
///
/// Some transformations may decide not to output a value (e.g. when a struct field is not found),
/// which is represented by returning `Ok(None)`. When composing transforms using [`Transform::then`]
/// and the first returned `Ok(None)`, then the second [`Transform`] will not be executed.
pub trait Transform {
    type Source: Array;
    type Target: Array;

    /// Apply the transformation to the source array.
    fn transform(&self, source: &Self::Source) -> Result<Option<Self::Target>, Error>;

    /// Chain this transformation with another transformation.
    fn then<T2>(self, next: T2) -> Then<Self, T2>
    where
        Self: Sized,
        T2: Transform<Source = Self::Target>,
    {
        Then {
            first: self,
            second: next,
        }
    }
}

impl<T> Transform for T
where
    T: Fn(&ListArray) -> Result<Option<ListArray>, Error>,
{
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &Self::Source) -> Result<Option<Self::Target>, Error> {
        (self)(source)
    }
}

/// Composed transformation created by calling [`.then()`](Transform::then).
#[derive(Clone)]
pub struct Then<T1, T2> {
    first: T1,
    second: T2,
}

impl<T1, T2, M> Transform for Then<T1, T2>
where
    T1: Transform<Target = M>,
    T2: Transform<Source = M>,
    M: Array,
{
    type Source = T1::Source;
    type Target = T2::Target;

    fn transform(&self, source: &Self::Source) -> Result<Option<Self::Target>, Error> {
        match self.first.transform(source)? {
            Some(mid) => self.second.transform(&mid),
            None => Ok(None),
        }
    }
}
