//! Transforms that apply operations to elements within arrays.

use std::sync::Arc;

use arrow::array::{
    Array, ArrowPrimitiveType, FixedSizeListArray, ListArray, PrimitiveArray, StringArray,
};
use arrow::datatypes::Field;

use crate::{Error, Transform};

/// Maps a transformation over the elements within a list array.
///
/// Applies the inner transformation to the flattened values array while preserving
/// the list structure (offsets and null bitmap).
#[derive(Clone)]
pub struct MapList<T> {
    transform: T,
}

impl<T> MapList<T> {
    /// Create a new list mapper that applies the given transformation to list elements.
    pub fn new(transform: T) -> Self {
        Self { transform }
    }
}

impl<T, S, U> Transform for MapList<T>
where
    T: Transform<Source = S, Target = U>,
    S: Array + 'static,
    U: Array + 'static,
{
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &ListArray) -> Result<ListArray, Error> {
        let values = source.values();
        let downcast =
            values
                .as_any()
                .downcast_ref::<S>()
                .ok_or_else(|| Error::UnexpectedListValueType {
                    expected: std::any::type_name::<S>().to_owned(),
                    actual: values.data_type().clone(),
                })?;

        let transformed = self.transform.transform(downcast)?;
        let new_field = Arc::new(Field::new_list_field(
            transformed.data_type().clone(),
            transformed.is_nullable(),
        ));

        let (_, offsets, _, nulls) = source.clone().into_parts();
        Ok(ListArray::new(
            new_field,
            offsets,
            Arc::new(transformed),
            nulls,
        ))
    }
}

/// Maps a transformation over the elements within a fixed-size list array.
///
/// Applies the inner transformation to the flattened values array while preserving
/// the fixed-size list structure (element count and null bitmap).
#[derive(Clone)]
pub struct MapFixedSizeList<T> {
    transform: T,
}

impl<T> MapFixedSizeList<T> {
    /// Create a new fixed-size list mapper that applies the given transformation to list elements.
    pub fn new(transform: T) -> Self {
        Self { transform }
    }
}

impl<T, S, U> Transform for MapFixedSizeList<T>
where
    T: Transform<Source = S, Target = U>,
    S: Array + 'static,
    U: Array + 'static,
{
    type Source = FixedSizeListArray;
    type Target = FixedSizeListArray;

    fn transform(&self, source: &FixedSizeListArray) -> Result<FixedSizeListArray, Error> {
        let values = source.values();
        let downcast = values.as_any().downcast_ref::<S>().ok_or_else(|| {
            Error::UnexpectedFixedSizeListValueType {
                expected: std::any::type_name::<S>().to_owned(),
                actual: values.data_type().clone(),
            }
        })?;

        let transformed = self.transform.transform(downcast)?;
        let field = Arc::new(Field::new_list_field(
            transformed.data_type().clone(),
            transformed.is_nullable(),
        ));
        let size = source.value_length();
        let nulls = source.nulls().cloned();

        Ok(FixedSizeListArray::new(
            field,
            size,
            Arc::new(transformed),
            nulls,
        ))
    }
}

/// Maps a function over each element in a primitive array.
///
/// Applies the given function to each non-null element, preserving null values.
/// Works with any Arrow primitive type.
#[derive(Clone)]
pub struct MapPrimitive<S, F, T = S>
where
    S: ArrowPrimitiveType,
    T: ArrowPrimitiveType,
    F: Fn(S::Native) -> T::Native,
{
    f: F,
    _phantom_source: std::marker::PhantomData<S>,
    _phantom_target: std::marker::PhantomData<T>,
}

impl<S, F, T> MapPrimitive<S, F, T>
where
    S: ArrowPrimitiveType,
    T: ArrowPrimitiveType,
    F: Fn(S::Native) -> T::Native,
{
    /// Create a new mapper that applies the given function to each element.
    pub fn new(f: F) -> Self {
        Self {
            f,
            _phantom_source: std::marker::PhantomData,
            _phantom_target: std::marker::PhantomData,
        }
    }
}

impl<S, F, T> Transform for MapPrimitive<S, F, T>
where
    S: ArrowPrimitiveType,
    T: ArrowPrimitiveType,
    F: Fn(S::Native) -> T::Native,
{
    type Source = PrimitiveArray<S>;
    type Target = PrimitiveArray<T>;

    fn transform(&self, source: &PrimitiveArray<S>) -> Result<PrimitiveArray<T>, Error> {
        let result: PrimitiveArray<T> = source.iter().map(|opt| opt.map(|v| (self.f)(v))).collect();
        Ok(result)
    }
}

/// Replaces null values in a primitive array with a specified default value.
///
/// All null entries in the source array will be replaced with the provided value,
/// while non-null entries remain unchanged.
#[derive(Clone)]
pub struct ReplaceNull<T>
where
    T: ArrowPrimitiveType,
{
    default_value: T::Native,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> ReplaceNull<T>
where
    T: ArrowPrimitiveType,
{
    /// Create a new null replacer with the given default value.
    pub fn new(default_value: T::Native) -> Self {
        Self {
            default_value,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T> Transform for ReplaceNull<T>
where
    T: ArrowPrimitiveType,
{
    type Source = PrimitiveArray<T>;
    type Target = PrimitiveArray<T>;

    fn transform(&self, source: &PrimitiveArray<T>) -> Result<PrimitiveArray<T>, Error> {
        let result: PrimitiveArray<T> = source
            .iter()
            .map(|opt| Some(opt.unwrap_or(self.default_value)))
            .collect();
        Ok(result)
    }
}

/// Prepends a prefix to each string value in a string array.
///
/// Null values are preserved.
#[derive(Clone)]
pub struct StringPrefix {
    prefix: String,
}

impl StringPrefix {
    /// Create a new string prefix prepender.
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

impl Transform for StringPrefix {
    type Source = StringArray;
    type Target = StringArray;

    fn transform(&self, source: &StringArray) -> Result<StringArray, Error> {
        let result: StringArray = source
            .iter()
            .map(|opt| opt.map(|s| format!("{}{}", self.prefix, s)))
            .collect();
        Ok(result)
    }
}

/// Appends a suffix to each string value in a string array.
///
/// Null values are preserved.
#[derive(Clone)]
pub struct StringSuffix {
    suffix: String,
}

impl StringSuffix {
    /// Create a new string suffix appender.
    pub fn new(suffix: impl Into<String>) -> Self {
        Self {
            suffix: suffix.into(),
        }
    }
}

impl Transform for StringSuffix {
    type Source = StringArray;
    type Target = StringArray;

    fn transform(&self, source: &StringArray) -> Result<StringArray, Error> {
        let result: StringArray = source
            .iter()
            .map(|opt| opt.map(|s| format!("{}{}", s, self.suffix)))
            .collect();
        Ok(result)
    }
}
