use polars_core::prelude::*;
use re_log_types::{
    external::arrow2_convert::{field::ArrowField, serialize::ArrowSerialize},
    msg_bundle::Component,
};

pub fn df_builder1<C0>(c0: &Vec<Option<C0>>) -> DataFrame
where
    C0: Component + 'static,
    Option<C0>: ArrowSerialize + ArrowField<Type = Option<C0>>,
{
    use arrow2::array::MutableArray;
    use re_log_types::external::arrow2_convert::serialize::arrow_serialize_to_mutable_array;

    let array0 = arrow_serialize_to_mutable_array::<Option<C0>, Option<C0>, &Vec<Option<C0>>>(c0);

    let series0 = Series::try_from((C0::NAME, array0.unwrap().as_box())).unwrap();

    DataFrame::new(vec![series0]).unwrap()
}

pub fn df_builder2<C0, C1>(c0: &Vec<Option<C0>>, c1: &Vec<Option<C1>>) -> DataFrame
where
    C0: Component + 'static,
    Option<C0>: ArrowSerialize + ArrowField<Type = Option<C0>>,
    C1: Component + 'static,
    Option<C1>: ArrowSerialize + ArrowField<Type = Option<C1>>,
{
    use arrow2::array::MutableArray;
    use re_log_types::external::arrow2_convert::serialize::arrow_serialize_to_mutable_array;

    let array0 = arrow_serialize_to_mutable_array::<Option<C0>, Option<C0>, &Vec<Option<C0>>>(c0);
    let array1 = arrow_serialize_to_mutable_array::<Option<C1>, Option<C1>, &Vec<Option<C1>>>(c1);

    let series0 = Series::try_from((C0::NAME, array0.unwrap().as_box())).unwrap();
    let series1 = Series::try_from((C1::NAME, array1.unwrap().as_box())).unwrap();

    DataFrame::new(vec![series0, series1]).unwrap()
}

pub fn df_builder3<C0, C1, C2>(
    c0: &Vec<Option<C0>>,
    c1: &Vec<Option<C1>>,
    c2: &Vec<Option<C2>>,
) -> DataFrame
where
    C0: Component + 'static,
    Option<C0>: ArrowSerialize + ArrowField<Type = Option<C0>>,
    C1: Component + 'static,
    Option<C1>: ArrowSerialize + ArrowField<Type = Option<C1>>,
    C2: Component + 'static,
    Option<C2>: ArrowSerialize + ArrowField<Type = Option<C2>>,
{
    use arrow2::array::MutableArray;
    use re_log_types::external::arrow2_convert::serialize::arrow_serialize_to_mutable_array;

    let array0 = arrow_serialize_to_mutable_array::<Option<C0>, Option<C0>, &Vec<Option<C0>>>(c0);
    let array1 = arrow_serialize_to_mutable_array::<Option<C1>, Option<C1>, &Vec<Option<C1>>>(c1);
    let array2 = arrow_serialize_to_mutable_array::<Option<C2>, Option<C2>, &Vec<Option<C2>>>(c2);

    let series0 = Series::try_from((C0::NAME, array0.unwrap().as_box())).unwrap();
    let series1 = Series::try_from((C1::NAME, array1.unwrap().as_box())).unwrap();
    let series2 = Series::try_from((C2::NAME, array2.unwrap().as_box())).unwrap();

    DataFrame::new(vec![series0, series1, series2]).unwrap()
}

pub fn compare_df(df1: &DataFrame, df2: &DataFrame) {
    let mut cols1 = df1.get_column_names();
    cols1.sort();
    let mut cols2 = df2.get_column_names();
    cols2.sort();

    assert_eq!(df1.select(cols1).unwrap(), df2.select(cols2).unwrap());
}
