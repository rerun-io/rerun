use std::borrow::Borrow;

use polars::{prelude::*, series::IsSorted};

mod arrow_log_db;

pub use arrow_log_db::LogDb;
use re_log::warn;

/// Find the "Rerun-latest" index in `col` that matches `time`. Returns None if `time` is before any values.
pub fn time_index(df: &DataFrame, col: &str, time: i64) -> Result<Option<usize>, PolarsError> {
    let col = df.column(col)?;
    if col.is_sorted() == IsSorted::Not {
        warn!("DataFrame is not sorted on col {col}.");
    }

    let ary = col.cast(&DataType::Time).and_then(|t| t.time().cloned())?;

    let slice = ary.cont_slice()?;
    let x = slice
        .binary_search(&time)
        .map(|idx| Some(idx))
        .unwrap_or_else(|idx| idx.checked_sub(1));
    Ok(x)
}

/// Perform a Rerun time query on the dataframe.
pub fn time_query(df: &DataFrame, col: &str, time: i64) -> Result<DataFrame, PolarsError> {
    let row_idx = time_index(&df, "time", time)?;
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
pub fn append_unified(base: &mut DataFrame, other: &DataFrame) -> PolarsResult<()> {
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
        for other_col in other_cols.into_iter() {
            let mut new_col = Series::full_null(
                other_col.name(),
                base.height() - other_col.len(),
                other_col.dtype(),
            );
            new_col.append(other_col)?;
            base.get_columns_mut().push(new_col.clone());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_query() {
        let mut df1: DataFrame = df!(
            "time" => &[1, 3, 2],
            "numeric" => &[None, None, Some(3)],
            "object" => &[None, Some("b"), None],
            "dat" => &[Some(99), None, Some(66)],
        )
        .unwrap();
        dbg!(&df1);

        let df_sorted = df1.sort_in_place(&["time"], false).unwrap();
        dbg!(&df_sorted);
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

        dbg!(&df1);
    }
}
