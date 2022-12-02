#![allow(dead_code)]

use std::borrow::Borrow;

use polars::{prelude::*, series::IsSorted};
use re_log::warn;

// ---

/// Find the "Rerun-latest" index in `col` that matches `time`. Returns None if `time` is before any values.
pub fn time_index(df: &DataFrame, col: &str, time: i64) -> Result<Option<usize>, PolarsError> {
    let col = df.column(col)?;
    if col.is_sorted() == IsSorted::Not {
        warn!("DataFrame is not sorted on col {col}.");
    }
    let ary = col.cast(&DataType::Time).and_then(|t| t.time().cloned())?;
    let slice = ary.cont_slice()?;
    Ok(slice
        .binary_search(&time)
        .map_or_else(|idx| idx.checked_sub(1), Some))
}

/// Perform a Rerun time query on the dataframe.
pub fn time_query(df: &DataFrame, _col: &str, time: i64) -> Result<DataFrame, PolarsError> {
    let row_idx = time_index(df, "time", time)?;
    Ok(df
        .head(row_idx.map(|idx| idx + 1))
        .fill_null(FillNullStrategy::Forward(None))
        .unwrap()
        .tail(Some(1)))
}

/// Return a Vec of items found in both `base_set` and `other`
fn intersect<I: Eq>(mut base_set: Vec<I>, other: impl Iterator<Item = I>) -> Vec<I> {
    other.fold(Vec::new(), |mut common, i_other| {
        if let Some(pos) = base_set.iter().position(|i_base| i_base == &i_other) {
            common.push(base_set.remove(pos));
        }
        common
    })
}

/// Append `other` to `base`, unifiying the Schema to be a superset of both by null-filling.
pub fn append_unified<'base>(
    base: &'base mut DataFrame,
    other: &'_ DataFrame,
) -> PolarsResult<&'base DataFrame> {
    if base.schema() == other.schema() {
        base.get_columns_mut()
            .iter_mut()
            .zip(other.get_columns().iter())
            .for_each(|(left, right)| {
                left.append(right).expect("should not fail");
            });
    } else {
        let mut other_cols: Vec<_> = other.get_columns().iter().map(Borrow::borrow).collect();

        // First vstack all pre-existing cols
        for base_col in base.get_columns_mut().iter_mut() {
            if let Some(pos) = other_cols
                .iter()
                .position(|other_col| base_col.name() == other_col.name())
            {
                let other_col = other_cols.remove(pos);
                if base_col.dtype() == other_col.dtype() {
                    base_col.append(other_col).expect("should not fail");
                } else {
                    return Err(PolarsError::SchemaMisMatch(
                        format!(
                            "Column {} has mismatched dtype: {} vs {}.",
                            base_col.name(),
                            base_col.dtype(),
                            other_col.dtype(),
                        )
                        .into(),
                    ));
                }
            } else {
                // This column exists in base, but not other, so append nulls.
                base_col.append(&Series::full_null("", other.height(), base_col.dtype()))?;
            }
        }

        // Anything left in other_cols didn't exist in base, so hstack them as new columns.
        for other_col in other_cols {
            let mut new_col = Series::full_null(
                other_col.name(),
                base.height() - other_col.len(),
                other_col.dtype(),
            );
            new_col.append(other_col)?;
            base.get_columns_mut().push(new_col.clone());
        }
    }

    Ok(base)
}

#[cfg(test)]
mod tests {
    use arrow2::{
        array::{Array, Float32Array, ListArray, StructArray},
        buffer::Buffer,
    };
    use polars::export::arrow::datatypes::{DataType, Field};

    use super::*;

    fn build_struct_array(len: usize) -> StructArray {
        let data: Box<[_]> = (0..len).into_iter().map(|i| i as f32 / 10.0).collect();
        let x = Float32Array::from_slice(&data).boxed();
        let y = Float32Array::from_slice(&data).boxed();
        let w = Float32Array::from_slice(&data).boxed();
        let h = Float32Array::from_slice(&data).boxed();
        let fields = vec![
            Field::new("x", DataType::Float32, false),
            Field::new("y", DataType::Float32, false),
            Field::new("w", DataType::Float32, false),
            Field::new("h", DataType::Float32, false),
        ];
        StructArray::new(DataType::Struct(fields), vec![x, y, w, h], None)
    }

    fn build_rect_series(len: usize) -> Box<dyn Array> {
        let struct_ary = build_struct_array(len);
        let ary = ListArray::<i32>::from_data(
            ListArray::<i32>::default_datatype(struct_ary.data_type().clone()), // datatype
            Buffer::from(vec![0, len as i32]),                                  // offsets
            struct_ary.boxed(),                                                 // values
            None,                                                               // validity
        );
        ary.boxed()
    }

    #[test]
    fn tester() {
        let mut s0 = Series::try_from(("rects", build_rect_series(5))).unwrap();
        let s1 = Series::try_from(("rects", build_rect_series(2))).unwrap();
        s0.append(&s1).unwrap();
        let _df0 = s0.into_frame();
    }

    #[test]
    fn test_time_query() {
        let mut df1: DataFrame = df!(
            "time" => &[1, 3, 2],
            "numeric" => &[None, None, Some(3)],
            "object" => &[None, Some("b"), None],
            "dat" => &[Some(99), None, Some(66)],
        )
        .unwrap();

        let _df_sorted = df1.sort_in_place(["time"], false).unwrap();
    }

    #[test]
    fn test_append_unified() {
        let mut df1 = df!(
            "colA" => [1, 2, 3],
            "colB" => ["one", "two", "three"],
        )
        .unwrap();

        let df2 = df!(
            "colA" => [4, 5, 6],
            "colC" => [Some(0.0), Some(0.1), None],
        )
        .unwrap();

        append_unified(&mut df1, &df2).unwrap();
    }
}
