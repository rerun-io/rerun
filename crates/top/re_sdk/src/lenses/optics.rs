use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, FixedSizeListArray, Float32Array, Float32Builder, Float64Array,
    Float64Builder, ListArray, StructArray,
};
use arrow::datatypes::{DataType, Field};
use arrow::error::ArrowError;

// ## Observations
//
// * The number of rows is identical, so transforming the contents is an affine mapping.

// Core trait
trait ArrowLens: Clone {
    type Source: Array + Clone;
    type Target: Array + Clone;

    fn project(&self, source: &Self::Source) -> Result<Self::Target, ArrowError>;
    fn embed(&self, source: Self::Source, value: Self::Target) -> Result<Self::Source, ArrowError>;

    fn over<F>(&self, source: Self::Source, f: F) -> Result<Self::Source, ArrowError>
    where
        F: FnOnce(Self::Target) -> Result<Self::Target, ArrowError>,
    {
        let value = self.project(&source)?;
        self.embed(source, f(value)?)
    }
}

#[derive(Clone)]
struct Compose<L1, L2> {
    first: L1,
    second: L2,
}

impl<L1, L2, M> ArrowLens for Compose<L1, L2>
where
    L1: ArrowLens<Target = M>,
    L2: ArrowLens<Source = M>,
    M: Array,
{
    type Source = L1::Source;
    type Target = L2::Target;

    fn project(&self, source: &Self::Source) -> Result<Self::Target, ArrowError> {
        let mid = self.first.project(source)?;
        self.second.project(&mid)
    }

    fn embed(&self, source: Self::Source, value: Self::Target) -> Result<Self::Source, ArrowError> {
        let mid = self.first.project(&source)?;
        let new_mid = self.second.embed(mid, value)?;
        self.first.embed(source, new_mid)
    }
}

// Extension trait for ergonomic composition
trait LensExt: ArrowLens {
    fn then<L2>(self, next: L2) -> Compose<Self, L2>
    where
        Self: Sized,
        L2: ArrowLens<Source = Self::Target>,
    {
        Compose {
            first: self,
            second: next,
        }
    }
}

impl<T: ArrowLens> LensExt for T {}

// Lens: Extract field from StructArray
#[derive(Clone)]
struct StructFieldLens {
    field_name: String,
}

impl ArrowLens for StructFieldLens {
    type Source = StructArray;
    type Target = ArrayRef;

    fn project(&self, source: &StructArray) -> Result<ArrayRef, ArrowError> {
        source
            .column_by_name(&self.field_name)
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(format!(
                    "Field {} not found",
                    self.field_name
                ))
            })
            .map(Clone::clone)
    }

    fn embed(&self, source: StructArray, value: ArrayRef) -> Result<StructArray, ArrowError> {
        let (fields, arrays, nulls) = source.into_parts();
        let field_idx = fields
            .iter()
            .position(|f| f.name() == &self.field_name)
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(format!(
                    "Field {} not found",
                    self.field_name
                ))
            })?;

        let mut new_arrays = arrays.to_vec();
        new_arrays[field_idx] = value;

        Ok(StructArray::new(fields, new_arrays, nulls))
    }
}

// Lens: Transform each element in ListArray
#[derive(Clone)]
struct ListEachLens<L> {
    element_lens: L,
}

impl<L> ArrowLens for ListEachLens<L>
where
    L: ArrowLens,
    L::Source: Array + 'static,
    L::Target: Array + 'static,
{
    type Source = ListArray;
    type Target = ListArray;

    fn project(&self, source: &ListArray) -> Result<ListArray, ArrowError> {
        let values = source.values();
        let downcast = values.as_any().downcast_ref::<L::Source>().ok_or_else(|| {
            arrow::error::ArrowError::InvalidArgumentError("Type mismatch in ListEachLens".into())
        })?;

        let transformed = self.element_lens.project(downcast)?;
        let new_field = Arc::new(Field::new("item", transformed.data_type().clone(), true));

        let (_, offsets, _, nulls) = source.clone().into_parts();
        Ok(ListArray::new(
            new_field,
            offsets,
            Arc::new(transformed),
            nulls,
        ))
    }

    fn embed(&self, source: ListArray, value: ListArray) -> Result<ListArray, ArrowError> {
        let old_values = source
            .values()
            .as_any()
            .downcast_ref::<L::Source>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Type mismatch in ListEachLens".into(),
                )
            })?
            .clone();

        let new_values_raw = value.values();
        let new_values = new_values_raw
            .as_any()
            .downcast_ref::<L::Target>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Type mismatch in ListEachLens embed".into(),
                )
            })?
            .clone();

        let restored = self.element_lens.embed(old_values, new_values)?;
        let (field, offsets, _, nulls) = source.into_parts();
        Ok(ListArray::new(field, offsets, Arc::new(restored), nulls))
    }
}

// Lens: Convert struct {x, y} to FixedSizeListArray[2]
#[derive(Clone)]
struct PointStructToFixedListLens;

impl ArrowLens for PointStructToFixedListLens {
    type Source = StructArray;
    type Target = FixedSizeListArray;

    fn project(&self, source: &StructArray) -> Result<FixedSizeListArray, ArrowError> {
        let x_array = source
            .column_by_name("x")
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError("Missing x field".into())
            })?
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError("x must be Float64".into())
            })?;

        let y_array = source
            .column_by_name("y")
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError("Missing y field".into())
            })?
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError("y must be Float64".into())
            })?;

        let len = source.len();
        let mut builder = Float64Builder::with_capacity(len * 2);

        for i in 0..len {
            if source.is_null(i) {
                builder.append_null();
                builder.append_null();
            } else {
                if x_array.is_null(i) || y_array.is_null(i) {
                    builder.append_null();
                    builder.append_null();
                } else {
                    builder.append_value(x_array.value(i));
                    builder.append_value(y_array.value(i));
                }
            }
        }

        let values = builder.finish();
        let field = Arc::new(Field::new_list_field(DataType::Float64, true));

        // Build null buffer from source
        let null_buffer = source.nulls().cloned();

        Ok(FixedSizeListArray::new(
            field,
            2,
            Arc::new(values),
            null_buffer,
        ))
    }

    fn embed(
        &self,
        _source: StructArray,
        _value: FixedSizeListArray,
    ) -> Result<StructArray, ArrowError> {
        todo!();
    }
}

// Lens: Transform ListArray of structs to ListArray of FixedSizeLists
#[derive(Clone)]
struct ListStructToFixedListLens;

impl ArrowLens for ListStructToFixedListLens {
    type Source = ListArray;
    type Target = ListArray;

    fn project(&self, source: &ListArray) -> Result<ListArray, ArrowError> {
        let struct_array = source
            .values()
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Expected ListArray of StructArray".into(),
                )
            })?;

        let fixed_list = PointStructToFixedListLens.project(struct_array)?;
        let field = Arc::new(Field::new("item", fixed_list.data_type().clone(), true));
        let (_, offsets, _, nulls) = source.clone().into_parts();

        Ok(ListArray::new(field, offsets, Arc::new(fixed_list), nulls))
    }

    fn embed(&self, source: ListArray, _value: ListArray) -> Result<ListArray, ArrowError> {
        Ok(source) // Simplified for now
    }
}

// Lens: Unwrap single-element ListArray and extract field from the struct
#[derive(Clone)]
struct UnwrapSingleStructFieldLens {
    field_name: String,
}

impl ArrowLens for UnwrapSingleStructFieldLens {
    type Source = ListArray;
    type Target = ListArray;

    fn project(&self, source: &ListArray) -> Result<ListArray, ArrowError> {
        // Get the struct array from the list values
        let struct_array = source
            .values()
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Expected ListArray of StructArray".into(),
                )
            })?;

        // Extract the field
        let field_array = struct_array
            .column_by_name(&self.field_name)
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(format!(
                    "Field {} not found",
                    self.field_name
                ))
            })?;

        // Downcast to ListArray
        field_array
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(format!(
                    "Field {} is not a ListArray",
                    self.field_name
                ))
            })
            .map(Clone::clone)
    }

    fn embed(&self, _source: ListArray, _value: ListArray) -> Result<ListArray, ArrowError> {
        todo!()
    }
}

// Lens: Transform Float64Array values
#[derive(Clone)]
struct Float64MapLens<F>
where
    F: Fn(f64) -> f64 + Clone,
{
    f: F,
}

impl<F> ArrowLens for Float64MapLens<F>
where
    F: Fn(f64) -> f64 + Clone,
{
    type Source = Float64Array;
    type Target = Float64Array;

    fn project(&self, source: &Float64Array) -> Result<Float64Array, ArrowError> {
        let mut builder = Float64Builder::with_capacity(source.len());
        for i in 0..source.len() {
            if source.is_null(i) {
                builder.append_null();
            } else {
                builder.append_value((self.f)(source.value(i)));
            }
        }
        Ok(builder.finish())
    }

    fn embed(&self, _source: Float64Array, value: Float64Array) -> Result<Float64Array, ArrowError> {
        Ok(value)
    }
}

// Lens: Convert Float64Array to Float32Array
#[derive(Clone)]
struct Float64ToFloat32Lens;

impl ArrowLens for Float64ToFloat32Lens {
    type Source = Float64Array;
    type Target = Float32Array;

    fn project(&self, source: &Float64Array) -> Result<Float32Array, ArrowError> {
        let mut builder = Float32Builder::with_capacity(source.len());
        for i in 0..source.len() {
            if source.is_null(i) {
                builder.append_null();
            } else {
                builder.append_value(source.value(i) as f32);
            }
        }
        Ok(builder.finish())
    }

    fn embed(&self, _source: Float64Array, value: Float32Array) -> Result<Float64Array, ArrowError> {
        let mut builder = Float64Builder::with_capacity(value.len());
        for i in 0..value.len() {
            if value.is_null(i) {
                builder.append_null();
            } else {
                builder.append_value(value.value(i) as f64);
            }
        }
        Ok(builder.finish())
    }
}

// Lens: Transform FixedSizeListArray values
#[derive(Clone)]
struct FixedSizeListMapLens<L> {
    inner_lens: L,
}

impl<L> ArrowLens for FixedSizeListMapLens<L>
where
    L: ArrowLens,
    L::Source: Array + 'static,
    L::Target: Array + 'static,
{
    type Source = FixedSizeListArray;
    type Target = FixedSizeListArray;

    fn project(&self, source: &FixedSizeListArray) -> Result<FixedSizeListArray, ArrowError> {
        let values = source
            .values()
            .as_any()
            .downcast_ref::<L::Source>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Type mismatch in FixedSizeListMapLens".into(),
                )
            })?;

        let transformed = self.inner_lens.project(values)?;
        let field = Arc::new(Field::new("item", transformed.data_type().clone(), true));
        let size = source.value_length();
        let nulls = source.nulls().cloned();

        Ok(FixedSizeListArray::new(
            field,
            size,
            Arc::new(transformed),
            nulls,
        ))
    }

    fn embed(
        &self,
        source: FixedSizeListArray,
        value: FixedSizeListArray,
    ) -> Result<FixedSizeListArray, ArrowError> {
        let old_values = source
            .values()
            .as_any()
            .downcast_ref::<L::Source>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Type mismatch in FixedSizeListMapLens embed".into(),
                )
            })?
            .clone();

        let new_values = value
            .values()
            .as_any()
            .downcast_ref::<L::Target>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Type mismatch in FixedSizeListMapLens embed".into(),
                )
            })?
            .clone();

        let restored = self.inner_lens.embed(old_values, new_values)?;
        let (field, size, _, nulls) = source.into_parts();

        Ok(FixedSizeListArray::new(
            field,
            size,
            Arc::new(restored),
            nulls,
        ))
    }
}


// Need a lens to map over the outer ListArray's elements
#[derive(Clone)]
struct ListMapLens<L> {
    inner_lens: L,
}

impl<L> ArrowLens for ListMapLens<L>
where
    L: ArrowLens,
    L::Source: Array + 'static,
    L::Target: Array + 'static,
{
    type Source = ListArray;
    type Target = ListArray;

    fn project(&self, source: &ListArray) -> Result<ListArray, ArrowError> {
        let values = source
            .values()
            .as_any()
            .downcast_ref::<L::Source>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Type mismatch in ListMapLens".into(),
                )
            })?;

        let transformed = self.inner_lens.project(values)?;
        let new_field = Arc::new(Field::new("item", transformed.data_type().clone(), true));
        let (_, offsets, _, nulls) = source.clone().into_parts();

        Ok(ListArray::new(
            new_field,
            offsets,
            Arc::new(transformed),
            nulls,
        ))
    }

    fn embed(&self, source: ListArray, value: ListArray) -> Result<ListArray, ArrowError> {
        let old_values = source
            .values()
            .as_any()
            .downcast_ref::<L::Source>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Type mismatch in ListMapLens embed".into(),
                )
            })?
            .clone();

        let new_values = value
            .values()
            .as_any()
            .downcast_ref::<L::Target>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Type mismatch in ListMapLens embed".into(),
                )
            })?
            .clone();

        let restored = self.inner_lens.embed(old_values, new_values)?;
        let (field, offsets, _, nulls) = source.into_parts();
        Ok(ListArray::new(field, offsets, Arc::new(restored), nulls))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use arrow::{
        array::{
            ArrayRef, Float64Builder, ListArray, ListBuilder, RecordBatch, RecordBatchOptions,
            StructBuilder,
        },
        datatypes::{DataType, Field, Fields, Schema},
    };

    /// Helper function to wrap an [`ArrayRef`] into a [`RecordBatch`] for easier printing.
    fn wrap_in_record_batch(array: ArrayRef) -> RecordBatch {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![Field::new("col", array.data_type().clone(), true)],
            Default::default(),
        ));
        RecordBatch::try_new_with_options(schema, vec![array], &RecordBatchOptions::default())
            .unwrap()
    }

    struct DisplayRB(RecordBatch);

    impl From<ListArray> for DisplayRB {
        fn from(array: ListArray) -> Self {
            Self(wrap_in_record_batch(Arc::new(array)))
        }
    }

    impl std::fmt::Display for DisplayRB {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", re_format_arrow::format_record_batch(&self.0))
        }
    }

    fn create_nasty_component_column() -> ListArray {
        let inner_struct_fields = Fields::from(vec![
            Field::new("x", DataType::Float64, false),
            Field::new("y", DataType::Float64, false),
        ]);

        // Middle struct schema: {poses: List<Struct<x: Float32>>}
        let middle_struct_fields = Fields::from(vec![Field::new(
            "poses",
            DataType::List(Arc::new(Field::new(
                "item",
                DataType::Struct(inner_struct_fields.clone()),
                false,
            ))),
            false,
        )]);

        // Construct nested builders
        let inner_struct_builder = StructBuilder::new(
            inner_struct_fields.clone(),
            vec![
                Box::new(Float64Builder::new()),
                Box::new(Float64Builder::new()),
            ],
        );

        let list_builder = ListBuilder::new(inner_struct_builder).with_field(Arc::new(Field::new(
            "item",
            DataType::Struct(inner_struct_fields),
            false,
        )));

        let struct_builder = StructBuilder::new(middle_struct_fields, vec![Box::new(list_builder)]);

        let mut column_builder = ListBuilder::new(struct_builder);

        // Row 0:
        let struct_val = column_builder.values();
        let list = struct_val
            .field_builder::<ListBuilder<StructBuilder>>(0)
            .unwrap();
        let inner = list.values();
        inner
            .field_builder::<Float64Builder>(0)
            .unwrap()
            .append_value(0.0);
        inner
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(0.0);
        inner.append(true);
        inner
            .field_builder::<Float64Builder>(0)
            .unwrap()
            .append_value(42.0);
        inner
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(42.0);
        inner.append(true);
        list.append(true);
        struct_val.append(true);
        column_builder.append(true);

        // Row 1:
        let struct_val = column_builder.values();
        let list = struct_val
            .field_builder::<ListBuilder<StructBuilder>>(0)
            .unwrap();
        let inner = list.values();
        inner
            .field_builder::<Float64Builder>(0)
            .unwrap()
            .append_value(17.0);
        inner
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(17.0);
        inner.append(true);
        list.append(true);
        struct_val.append(true);
        column_builder.append(true);

        // Row 2:
        let struct_val = column_builder.values();
        struct_val
            .field_builder::<ListBuilder<StructBuilder>>(0)
            .unwrap()
            .append(true);
        struct_val.append(true);
        column_builder.append(true);

        // Row 3:
        column_builder.append(false);

        column_builder.finish()
    }

    #[test]
    fn simple() {
        let array = create_nasty_component_column();
        println!("{}", DisplayRB::from(array.clone()));

        let pipeline = UnwrapSingleStructFieldLens {
            field_name: "poses".into(),
        }
        .then(ListStructToFixedListLens);

        let result: ListArray = pipeline.project(&array).unwrap();

        insta::assert_snapshot!("simple", format!("{}", DisplayRB::from(result.clone())));
    }

    #[test]
    fn add_one_to_leaves() {
        let array = create_nasty_component_column();

        let pipeline = UnwrapSingleStructFieldLens {
            field_name: "poses".into(),
        }
        .then(ListStructToFixedListLens)
        .then(ListMapLens {
            inner_lens: FixedSizeListMapLens {
                inner_lens: Float64MapLens { f: |x| x + 1.0 },
            },
        });

        let result = pipeline.project(&array).unwrap();

        insta::assert_snapshot!("add_one_to_leaves", format!("{}", DisplayRB::from(result)));
    }

    #[test]
    fn convert_to_f32() {
        let array = create_nasty_component_column();

        let pipeline = UnwrapSingleStructFieldLens {
            field_name: "poses".into(),
        }
        .then(ListStructToFixedListLens)
        .then(ListMapLens {
            inner_lens: FixedSizeListMapLens {
                inner_lens: Float64ToFloat32Lens,
            },
        });

        let result = pipeline.project(&array).unwrap();

        insta::assert_snapshot!("convert_to_f32", format!("{}", DisplayRB::from(result)));
    }
}
