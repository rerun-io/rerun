use arrow2::{
    array::{
        growable::make_growable, Array, FixedSizeListArray, ListArray, StructArray, UnionArray,
    },
    bitmap::Bitmap,
    datatypes::{DataType, Field, UnionMode},
    offset::Offsets,
};
use itertools::Itertools;

// ---

pub trait ArrayExt: Array {
    /// Returns the length of the child array at the given index.
    ///
    /// * Panics if `self` is not a `ListArray<i32>`.
    /// * Panics if `child_nr` is out of bounds.
    fn get_child_length(&self, child_nr: usize) -> usize;

    /// Create a new `Array` which avoids problematic types for polars.
    ///
    /// This does the following conversion:
    ///  - `FixedSizeList` -> `List`
    ///  - `Union` -> `Struct`
    ///
    /// Nested types are expanded and cleaned recursively
    fn clean_for_polars(&self) -> Box<dyn Array>;
}

impl ArrayExt for dyn Array {
    /// Return the length of the first child.
    ///
    /// ## Panics
    ///
    /// Panics if `Self` is not a `ListArray<i32>`, or if the array is empty (no children).
    fn get_child_length(&self, child_nr: usize) -> usize {
        self.as_any()
            .downcast_ref::<ListArray<i32>>()
            .unwrap()
            .offsets()
            .lengths()
            .nth(child_nr)
            .unwrap()
    }

    /// Create a new `Array` which avoids problematic types for polars.
    ///
    /// This does the following conversion:
    ///  - `FixedSizeList` -> `List`
    ///  - `Union` -> `Struct`
    ///
    /// Nested types are expanded and cleaned recursively
    fn clean_for_polars(&self) -> Box<dyn Array> {
        let datatype = self.data_type();
        let datatype = if let DataType::Extension(_, inner, _) = datatype {
            (**inner).clone()
        } else {
            datatype.clone()
        };

        match &datatype {
            DataType::List(field) => {
                // Recursively clean the contents
                let typed_arr = self.as_any().downcast_ref::<ListArray<i32>>().unwrap();
                let clean_vals = typed_arr.values().as_ref().clean_for_polars();
                let clean_data = DataType::List(Box::new(Field::new(
                    &field.name,
                    clean_vals.data_type().clone(),
                    field.is_nullable,
                )));
                ListArray::<i32>::try_new(
                    clean_data,
                    typed_arr.offsets().clone(),
                    clean_vals,
                    typed_arr.validity().cloned(),
                )
                .unwrap()
                .boxed()
            }
            DataType::LargeList(field) => {
                // Recursively clean the contents
                let typed_arr = self.as_any().downcast_ref::<ListArray<i64>>().unwrap();
                let clean_vals = typed_arr.values().as_ref().clean_for_polars();
                let clean_data = DataType::LargeList(Box::new(Field::new(
                    &field.name,
                    clean_vals.data_type().clone(),
                    field.is_nullable,
                )));
                ListArray::<i64>::try_new(
                    clean_data,
                    typed_arr.offsets().clone(),
                    clean_vals,
                    typed_arr.validity().cloned(),
                )
                .unwrap()
                .boxed()
            }
            DataType::FixedSizeList(field, len) => {
                // Recursively clean the contents and convert `FixedSizeListArray` -> `ListArray`
                let typed_arr = self.as_any().downcast_ref::<FixedSizeListArray>().unwrap();
                let clean_vals = typed_arr.values().as_ref().clean_for_polars();
                let clean_data = DataType::List(Box::new(Field::new(
                    &field.name,
                    clean_vals.data_type().clone(),
                    field.is_nullable,
                )));
                let lengths = std::iter::repeat(len).take(typed_arr.len()).cloned();
                let offsets = Offsets::try_from_lengths(lengths).unwrap();
                ListArray::<i32>::try_new(
                    clean_data,
                    offsets.into(),
                    clean_vals,
                    typed_arr.validity().cloned(),
                )
                .unwrap()
                .boxed()
            }
            DataType::Struct(fields) => {
                // Recursively clean the contents
                let typed_arr = self.as_any().downcast_ref::<StructArray>().unwrap();
                let clean_vals = typed_arr
                    .values()
                    .iter()
                    .map(|v| v.as_ref().clean_for_polars())
                    .collect_vec();
                let clean_fields = itertools::izip!(fields, &clean_vals)
                    .map(|(f, v)| Field::new(&f.name, v.data_type().clone(), f.is_nullable))
                    .collect_vec();
                let clean_data = DataType::Struct(clean_fields);
                StructArray::try_new(clean_data, clean_vals, typed_arr.validity().cloned())
                    .unwrap()
                    .boxed()
            }
            DataType::Union(fields, ids, UnionMode::Dense) => {
                // Recursively clean the contents and convert `UnionArray` -> `StructArray`
                let typed_arr = self.as_any().downcast_ref::<UnionArray>().unwrap();

                // Note: Union calls its stored value-arrays "fields"
                let clean_vals = typed_arr
                    .fields()
                    .iter()
                    .map(|v| v.as_ref().clean_for_polars())
                    .collect_vec();

                let ids = ids
                    .clone()
                    .unwrap_or_else(|| (0i32..(clean_vals.len() as i32)).collect_vec());

                // For Dense Unions, the value-arrays need to be padded to the
                // correct length, which we do by growing using the existing type
                // table.
                let padded_vals = itertools::izip!(&clean_vals, &ids)
                    .map(|(dense, id)| {
                        let mut next = 0;
                        let mut grow = make_growable(&[dense.as_ref()], true, self.len());
                        typed_arr.types().iter().for_each(|t| {
                            if *t == *id as i8 {
                                grow.extend(0, next, 1);
                                next += 1;
                            } else {
                                grow.extend_validity(1);
                            }
                        });
                        grow.as_box()
                    })
                    .collect_vec();

                let clean_field_types = itertools::izip!(fields, &clean_vals)
                    .map(|(f, v)| Field::new(&f.name, v.data_type().clone(), f.is_nullable))
                    .collect_vec();

                // The new type will be a struct
                let clean_data = DataType::Struct(clean_field_types);

                StructArray::try_new(clean_data, padded_vals, typed_arr.validity().cloned())
                    .unwrap()
                    .boxed()
            }
            DataType::Union(fields, ids, UnionMode::Sparse) => {
                // Recursively clean the contents and convert `UnionArray` -> `StructArray`
                let typed_arr = self.as_any().downcast_ref::<UnionArray>().unwrap();

                // Note: Union calls its stored value-arrays "fields"
                let clean_vals = typed_arr
                    .fields()
                    .iter()
                    .map(|v| v.as_ref().clean_for_polars())
                    .collect_vec();

                let ids = ids
                    .clone()
                    .unwrap_or_else(|| (0i32..(clean_vals.len() as i32)).collect_vec());

                // For Sparse Unions, the value-arrays is already the right
                // correct length, but should have a validity derived from the types array.
                let padded_vals = itertools::izip!(&clean_vals, &ids)
                    .map(|(sparse, id)| {
                        let validity = Bitmap::from(
                            typed_arr
                                .types()
                                .iter()
                                .map(|t| *t == *id as i8)
                                .collect_vec(),
                        );
                        sparse.with_validity(Some(validity))
                    })
                    .collect_vec();

                let clean_field_types = itertools::izip!(fields, &clean_vals)
                    .map(|(f, v)| Field::new(&f.name, v.data_type().clone(), f.is_nullable))
                    .collect_vec();

                // The new type will be a struct
                let clean_data = DataType::Struct(clean_field_types);

                StructArray::try_new(clean_data, padded_vals, typed_arr.validity().cloned())
                    .unwrap()
                    .boxed()
            }
            _ => self.to_boxed(),
        }
    }
}

#[test]
fn test_clean_for_polars_nomodify() {
    use re_log_types::datagen::build_some_colors;
    use re_log_types::DataCell;

    // Colors don't need polars cleaning
    let cell: DataCell = build_some_colors(5).try_into().unwrap();
    let cleaned = cell.as_arrow_ref().clean_for_polars();
    assert_eq!(cell.as_arrow_ref(), &*cleaned);
}

#[cfg(test)]
mod tests {
    use arrow2::datatypes::{DataType, Field, UnionMode};
    use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};
    use re_log_types::{component_types::Vec3D, Component, DataCell};

    use crate::ArrayExt;

    #[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
    #[arrow_field(type = "dense")]
    enum TestComponentWithUnionAndFixedSizeList {
        Bool(bool),
        Vec3D(Vec3D),
    }

    impl Component for TestComponentWithUnionAndFixedSizeList {
        fn name() -> re_log_types::ComponentName {
            "test_component_with_union_and_fixed_size_list".into()
        }
    }

    #[test]
    fn test_clean_for_polars_modify() {
        // Pick a type with both Unions and FixedSizeLists
        let elements = vec![TestComponentWithUnionAndFixedSizeList::Bool(false)];

        let cell: DataCell = elements.try_into().unwrap();
        assert_eq!(
            *cell.datatype(),
            DataType::Union(
                vec![
                    Field::new("Bool", DataType::Boolean, false),
                    Field::new(
                        "Vec3D",
                        DataType::FixedSizeList(
                            Box::new(Field::new("item", DataType::Float32, false)),
                            3
                        ),
                        false
                    )
                ],
                None,
                UnionMode::Dense
            )
        );

        let cleaned = cell.as_arrow_ref().clean_for_polars();

        assert_eq!(
            *cleaned.data_type(),
            DataType::Struct(vec![
                Field::new("Bool", DataType::Boolean, false),
                Field::new(
                    "Vec3D",
                    DataType::List(Box::new(Field::new("item", DataType::Float32, false))),
                    false
                )
            ],)
        );
    }
}
