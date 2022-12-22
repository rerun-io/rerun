#[cfg(feature = "polars")]
use polars_core::prelude::*;

#[cfg(feature = "polars")]
pub fn compare_df(df1: &DataFrame, df2: &DataFrame) {
    let mut cols1 = df1.get_column_names();
    cols1.sort();
    let mut cols2 = df2.get_column_names();
    cols2.sort();

    assert_eq!(df1.select(cols1).unwrap(), df2.select(cols2).unwrap());
}
