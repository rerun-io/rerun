use anyhow::bail;
use arrow2::{
    array::{
        growable::make_growable, Array, FixedSizeListArray, ListArray, PrimitiveArray, StructArray,
        UnionArray,
    },
    bitmap::Bitmap,
    datatypes::{DataType, Field, UnionMode},
    offset::Offsets,
    types::NativeType,
};
use itertools::Itertools;

// ---

pub trait ArrayExt: Array {
    /// Returns `true` if the array is dense (no nulls).
    fn is_dense(&self) -> bool;

    /// Returns `true` if the array is both sorted (increasing order) and contains only unique
    /// values.
    ///
    /// The array must be dense, otherwise the result of this method is undefined.
    fn is_sorted_and_unique(&self) -> anyhow::Result<bool>;

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
    fn is_dense(&self) -> bool {
        if let Some(validity) = self.validity() {
            validity.unset_bits() == 0
        } else {
            true
        }
    }

    fn is_sorted_and_unique(&self) -> anyhow::Result<bool> {
        debug_assert!(self.is_dense());

        fn is_sorted_and_unique_primitive<T: NativeType + PartialOrd>(arr: &dyn Array) -> bool {
            let values = arr.as_any().downcast_ref::<PrimitiveArray<T>>().unwrap();
            values.values().windows(2).all(|v| v[0] < v[1])
        }

        // TODO(cmc): support more datatypes as the need arise.
        match self.data_type() {
            DataType::Int8 => Ok(is_sorted_and_unique_primitive::<i8>(self)),
            DataType::Int16 => Ok(is_sorted_and_unique_primitive::<i16>(self)),
            DataType::Int32 => Ok(is_sorted_and_unique_primitive::<i32>(self)),
            DataType::Int64 => Ok(is_sorted_and_unique_primitive::<i64>(self)),
            DataType::UInt8 => Ok(is_sorted_and_unique_primitive::<u8>(self)),
            DataType::UInt16 => Ok(is_sorted_and_unique_primitive::<u16>(self)),
            DataType::UInt32 => Ok(is_sorted_and_unique_primitive::<u32>(self)),
            DataType::UInt64 => Ok(is_sorted_and_unique_primitive::<u64>(self)),
            DataType::Float32 => Ok(is_sorted_and_unique_primitive::<f32>(self)),
            DataType::Float64 => Ok(is_sorted_and_unique_primitive::<f64>(self)),
            _ => bail!("unsupported datatype: {:?}", self.data_type()),
        }
    }

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
        match self.data_type() {
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
    use re_log_types::msg_bundle::ComponentBundle;

    // Colors don't need polars cleaning
    let bundle: ComponentBundle = build_some_colors(5).try_into().unwrap();
    let cleaned = bundle.value_boxed().clean_for_polars();
    assert_eq!(bundle.value_boxed(), cleaned);
}

#[test]
fn test_clean_for_polars_modify() {
    use re_log_types::msg_bundle::ComponentBundle;
    use re_log_types::{Pinhole, Transform};
    // transforms are a nice pathological type with both Unions and FixedSizeLists
    let transforms = vec![Transform::Pinhole(Pinhole {
        image_from_cam: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]].into(),
        resolution: None,
    })];

    let bundle: ComponentBundle = transforms.try_into().unwrap();
    assert_eq!(
        *bundle.value_boxed().data_type(),
        DataType::List(Box::new(Field::new(
            "item",
            DataType::Union(
                vec![
                    Field::new("Unknown", DataType::Boolean, false),
                    Field::new(
                        "Rigid3",
                        DataType::Struct(vec![
                            Field::new(
                                "rotation",
                                DataType::FixedSizeList(
                                    Box::new(Field::new("item", DataType::Float32, false)),
                                    4
                                ),
                                false
                            ),
                            Field::new(
                                "translation",
                                DataType::FixedSizeList(
                                    Box::new(Field::new("item", DataType::Float32, false)),
                                    3
                                ),
                                false
                            )
                        ]),
                        false
                    ),
                    Field::new(
                        "Pinhole",
                        DataType::Struct(vec![
                            Field::new(
                                "image_from_cam",
                                DataType::FixedSizeList(
                                    Box::new(Field::new("item", DataType::Float32, false)),
                                    9
                                ),
                                false,
                            ),
                            Field::new(
                                "resolution",
                                DataType::FixedSizeList(
                                    Box::new(Field::new("item", DataType::Float32, false)),
                                    2
                                ),
                                true,
                            ),
                        ]),
                        false
                    )
                ],
                None,
                UnionMode::Dense
            ),
            true
        )))
    );

    let cleaned = bundle.value_boxed().clean_for_polars();

    assert_eq!(
        *cleaned.data_type(),
        DataType::List(Box::new(Field::new(
            "item",
            DataType::Struct(vec![
                Field::new("Unknown", DataType::Boolean, false),
                Field::new(
                    "Rigid3",
                    DataType::Struct(vec![
                        Field::new(
                            "rotation",
                            DataType::List(Box::new(Field::new("item", DataType::Float32, false)),),
                            false
                        ),
                        Field::new(
                            "translation",
                            DataType::List(Box::new(Field::new("item", DataType::Float32, false)),),
                            false
                        )
                    ]),
                    false
                ),
                Field::new(
                    "Pinhole",
                    DataType::Struct(vec![
                        Field::new(
                            "image_from_cam",
                            DataType::List(Box::new(Field::new("item", DataType::Float32, false))),
                            false,
                        ),
                        Field::new(
                            "resolution",
                            DataType::List(Box::new(Field::new("item", DataType::Float32, false))),
                            true,
                        ),
                    ]),
                    false
                )
            ],),
            true
        )))
    );
}
