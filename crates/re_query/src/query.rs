use polars::prelude::*;
use re_arrow_store::{DataStore, TimeQuery};
use re_log_types::{
    field_types::Instance, msg_bundle::Component, ComponentNameRef, ObjPath, Timeline,
};

#[derive(thiserror::Error, Debug)]
pub enum QueryError {
    #[error("Error executing Polars Query")]
    ArrowSerializationError(#[from] PolarsError),
}

pub type Result<T> = std::result::Result<T, QueryError>;

/// Retrieves a single component and its corresponding instance-ids
pub fn get_component_with_instance_ids(
    store: &DataStore,
    timeline: &Timeline,
    time_query: &TimeQuery,
    ent_path: &ObjPath,
    component: ComponentNameRef<'_>,
) -> Result<DataFrame> {
    let mut row_indices = [None, None];

    let components = [Instance::NAME, component];

    store.query(
        timeline,
        time_query,
        ent_path,
        component,
        &components,
        &mut row_indices,
    );

    let mut results = [None, None];
    store.get(&components, &row_indices, &mut results);

    let series: Vec<_> = components
        .iter()
        .zip(results)
        .filter_map(|(component, col)| col.map(|col| (component, col)))
        .map(|(&component, col)| Series::try_from((component, col)).unwrap())
        .collect();

    let df = DataFrame::new(series)?;

    Ok(df.explode(df.get_column_names())?)
}

/// Retrieve an entity as a polars Dataframe
///
/// An entity has a primary component which is expected to always be present.
/// The size of the batch will be equal to the size of the primary component.
///
/// The remainingComponents are joined based on the `Instance` id. If `Instance`
/// is not available, it's treated as an integer sequence of the correct length.
pub fn query_entity_with_primary<const N: usize>(
    store: &DataStore,
    timeline: &Timeline,
    time_query: &TimeQuery,
    ent_path: &ObjPath,
    primary: ComponentNameRef<'_>,
    components: &[ComponentNameRef<'_>; N],
) -> Result<DataFrame> {
    let df = get_component_with_instance_ids(store, timeline, time_query, ent_path, primary)?;

    // TODO(jleibs): lots of room for optimization here. Once "instance" is
    // guaranteed to be sorted we should be able to leverage this during the
    // join. Series have a SetSorted option to specify this. join_asof might be
    // the right place to start digging.

    let ldf = match df.column(Instance::NAME) {
        Ok(_) => df.lazy(),
        Err(_) => df
            .lazy()
            .with_row_count("row", None)
            .with_column(col("row").cast(DataType::UInt64).alias(Instance::NAME))
            .drop_columns(["row"]),
    };

    let joined = components
        .iter()
        .fold(ldf, |ldf, component| {
            let component =
                get_component_with_instance_ids(store, timeline, time_query, ent_path, component)
                    .unwrap();

            let lazy_component = match component.column(Instance::NAME) {
                Ok(_) => component.lazy(),
                Err(_) => component
                    .lazy()
                    .with_row_count("row", None)
                    .with_column(col("row").cast(DataType::UInt64).alias(Instance::NAME))
                    .drop_columns(["row"]),
            };

            ldf.left_join(lazy_component, col(Instance::NAME), col(Instance::NAME))
        })
        .collect()?;

    Ok(joined)
}

#[cfg(test)]
use re_log_types::external::arrow2_convert::{field::ArrowField, serialize::ArrowSerialize};

#[cfg(test)]
fn df_builder3<C0, C1, C2>(
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

#[cfg(test)]
fn compare_df(df1: &DataFrame, df2: &DataFrame) {
    let mut cols1 = df1.get_column_names();
    cols1.sort();
    let mut cols2 = df2.get_column_names();
    cols2.sort();

    assert_eq!(df1.select(cols1).unwrap(), df2.select(cols2).unwrap());
}

#[test]
fn simple_query() {
    use re_log_types::{
        datagen::build_frame_nr,
        field_types::{ColorRGBA, Instance, Point2D},
        msg_bundle::try_build_msg_bundle1,
        msg_bundle::try_build_msg_bundle2,
        MsgId,
    };

    let mut store = DataStore::default();

    let ent_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Create some points with an implicit index
    let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];
    let bundle = try_build_msg_bundle1(MsgId::random(), ent_path, timepoint, &points).unwrap();
    store.insert(&bundle).unwrap();

    // Assign one of them a color with an explicit index
    let color_instances = vec![Instance(1)];
    let colors = vec![ColorRGBA(0xff000000)];
    let bundle = try_build_msg_bundle2(
        MsgId::random(),
        ent_path,
        timepoint,
        (color_instances, colors),
    )
    .unwrap();
    store.insert(&bundle).unwrap();

    // Retrieve the view
    let timeline = timepoint[0].0;
    let time_query = re_arrow_store::TimeQuery::LatestAt(timepoint[0].1.as_i64());

    let df = query_entity_with_primary(
        &store,
        &timeline,
        &time_query,
        &ent_path.into(),
        Point2D::NAME,
        &[ColorRGBA::NAME],
    )
    .unwrap();

    // We expect this to generate the following `DataFrame`
    // ┌──────────┬───────────┬────────────┐
    // │ instance ┆ point2d   ┆ colorrgba  │
    // │ ---      ┆ ---       ┆ ---        │
    // │ u64      ┆ struct[2] ┆ u32        │
    // ╞══════════╪═══════════╪════════════╡
    // │ 0        ┆ {1.0,2.0} ┆ null       │
    // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1        ┆ {3.0,4.0} ┆ 4278190080 │
    // └──────────┴───────────┴────────────┘

    // Build expected df manually
    let instances = vec![Some(Instance(0)), Some(Instance(1))];
    let points = vec![
        Some(Point2D { x: 1.0, y: 2.0 }),
        Some(Point2D { x: 3.0, y: 4.0 }),
    ];
    let colors = vec![None, Some(ColorRGBA(0xff000000))];
    let expected = df_builder3(&instances, &points, &colors);

    // eprintln!("{:?}", expected);

    compare_df(&df, &expected);
}

#[test]
fn no_instance_join_query() {
    use re_log_types::{
        datagen::build_frame_nr,
        field_types::{ColorRGBA, Instance, Point2D},
        msg_bundle::try_build_msg_bundle1,
        MsgId,
    };

    let mut store = DataStore::default();

    let ent_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Create some points with an implicit index
    let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];
    let bundle = try_build_msg_bundle1(MsgId::random(), ent_path, timepoint, &points).unwrap();
    store.insert(&bundle).unwrap();

    // Assign one of them a color with an explicit index
    let colors = vec![ColorRGBA(0xff000000), ColorRGBA(0x00ff0000)];
    let bundle = try_build_msg_bundle1(MsgId::random(), ent_path, timepoint, &colors).unwrap();
    store.insert(&bundle).unwrap();

    // Retrieve the view
    let timeline = timepoint[0].0;
    let time_query = re_arrow_store::TimeQuery::LatestAt(timepoint[0].1.as_i64());

    let df = query_entity_with_primary(
        &store,
        &timeline,
        &time_query,
        &ent_path.into(),
        Point2D::NAME,
        &[ColorRGBA::NAME],
    )
    .unwrap();

    // We expect this to generate the following `DataFrame`
    // ┌──────────┬───────────┬────────────┐
    // │ instance ┆ point2d   ┆ colorrgba  │
    // │ ---      ┆ ---       ┆ ---        │
    // │ u64      ┆ struct[2] ┆ u32        │
    // ╞══════════╪═══════════╪════════════╡
    // │ 0        ┆ {1.0,2.0} ┆ 4278190080 │
    // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1        ┆ {3.0,4.0} ┆ 16711680   │
    // └──────────┴───────────┴────────────┘

    // Build expected df manually
    let instances = vec![Some(Instance(0)), Some(Instance(1))];
    let points = vec![
        Some(Point2D { x: 1.0, y: 2.0 }),
        Some(Point2D { x: 3.0, y: 4.0 }),
    ];
    let colors = vec![Some(ColorRGBA(0xff000000)), Some(ColorRGBA(0x00ff0000))];
    let expected = df_builder3(&instances, &points, &colors);

    //eprintln!("{:?}", df);
    //eprintln!("{:?}", expected);

    compare_df(&df, &expected);
}
