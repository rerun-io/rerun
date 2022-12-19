use polars_core::prelude::*;
use re_log_types::{
    external::arrow2_convert::{field::ArrowField, serialize::ArrowSerialize},
    msg_bundle::Component,
};

pub fn df_builder1<C0>(c0: &Vec<Option<C0>>) -> Result<DataFrame, PolarsError>
where
    C0: Component + 'static,
    Option<C0>: ArrowSerialize + ArrowField<Type = Option<C0>>,
{
    use arrow2::array::MutableArray;
    use re_log_types::external::arrow2_convert::serialize::arrow_serialize_to_mutable_array;

    let array0 = arrow_serialize_to_mutable_array::<Option<C0>, Option<C0>, &Vec<Option<C0>>>(c0);

    let series0 = Series::try_from((C0::name().as_str(), array0.unwrap().as_box()))?;

    DataFrame::new(vec![series0])
}

pub fn df_builder2<C0, C1>(
    c0: &Vec<Option<C0>>,
    c1: &Vec<Option<C1>>,
) -> Result<DataFrame, PolarsError>
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

    let series0 = Series::try_from((C0::name().as_str(), array0.unwrap().as_box()))?;
    let series1 = Series::try_from((C1::name().as_str(), array1.unwrap().as_box()))?;

    DataFrame::new(vec![series0, series1])
}

pub fn df_builder3<C0, C1, C2>(
    c0: &Vec<Option<C0>>,
    c1: &Vec<Option<C1>>,
    c2: &Vec<Option<C2>>,
) -> Result<DataFrame, PolarsError>
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

    let series0 = Series::try_from((C0::name().as_str(), array0.unwrap().as_box()))?;
    let series1 = Series::try_from((C1::name().as_str(), array1.unwrap().as_box()))?;
    let series2 = Series::try_from((C2::name().as_str(), array2.unwrap().as_box()))?;

    DataFrame::new(vec![series0, series1, series2])
}

#[test]
fn test_df_builder() {
    use re_log_types::field_types::{ColorRGBA, Point2D};

    let points = vec![
        Some(Point2D { x: 1.0, y: 2.0 }),
        Some(Point2D { x: 3.0, y: 4.0 }),
        Some(Point2D { x: 5.0, y: 6.0 }),
        Some(Point2D { x: 7.0, y: 8.0 }),
    ];

    let colors = vec![
        None,
        Some(ColorRGBA(0xff000000)),
        Some(ColorRGBA(0x00ff0000)),
        None,
    ];

    let df = df_builder2(&points, &colors).unwrap();
    // eprintln!("{:?}", df);
    //
    // ┌───────────┬────────────┐
    // │ point2d   ┆ colorrgba  │
    // │ ---       ┆ ---        │
    // │ struct[2] ┆ u32        │
    // ╞═══════════╪════════════╡
    // │ {1.0,2.0} ┆ null       │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ {3.0,4.0} ┆ 4278190080 │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ {5.0,6.0} ┆ 16711680   │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ {7.0,8.0} ┆ null       │
    // └───────────┴────────────┘

    // Unnesting the struct makes it easier to validate the results.
    let df = df.unnest([Point2D::name()]).unwrap();
    // eprintln!("{:?}", df);
    //
    // ┌─────┬─────┬────────────┐
    // │ x   ┆ y   ┆ colorrgba  │
    // │ --- ┆ --- ┆ ---        │
    // │ f32 ┆ f32 ┆ u32        │
    // ╞═════╪═════╪════════════╡
    // │ 1.0 ┆ 2.0 ┆ null       │
    // ├╌╌╌╌╌┼╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 3.0 ┆ 4.0 ┆ 4278190080 │
    // ├╌╌╌╌╌┼╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 5.0 ┆ 6.0 ┆ 16711680   │
    // ├╌╌╌╌╌┼╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 7.0 ┆ 8.0 ┆ null       │
    // └─────┴─────┴────────────┘

    let expected = df![
        "x" => &[1.0_f32, 3.0_f32, 5.0_f32, 7.0_f32],
        "y" => &[2.0_f32, 4.0_f32, 6.0_f32, 8.0_f32],
        ColorRGBA::name().as_str() => &[None, Some(0xff000000_u32), Some(0x00ff0000_u32), None ],
    ]
    .unwrap();

    assert_eq!(df, expected);
}
