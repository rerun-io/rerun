use polars_core::prelude::*;
use polars_lazy::prelude::*;
use re_arrow_store::{DataStore, TimelineQuery};
use re_log_types::{field_types::Instance, msg_bundle::Component, ComponentNameRef, ObjPath};

#[derive(thiserror::Error, Debug)]
pub enum QueryError {
    #[error("Could not find primary")]
    PrimaryNotFound,

    #[error("Error executing Polars Query")]
    PolarsError(#[from] PolarsError),
}

pub type Result<T> = std::result::Result<T, QueryError>;

/// Helper used to create an example store we can use for querying in doctests
pub fn __populate_example_store() -> DataStore {
    use re_log_types::{
        datagen::build_frame_nr,
        field_types::{ColorRGBA, Point2D},
        msg_bundle::try_build_msg_bundle2,
        MsgId,
    };

    let mut store = DataStore::default();

    let ent_path = "point";
    let timepoint = [build_frame_nr(123)];

    let instances = vec![Instance(42), Instance(96)];
    let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];

    let bundle =
        try_build_msg_bundle2(MsgId::random(), ent_path, timepoint, (&instances, &points)).unwrap();
    store.insert(&bundle).unwrap();

    let instances = vec![Instance(96)];
    let colors = vec![ColorRGBA(0xff000000)];
    let bundle =
        try_build_msg_bundle2(MsgId::random(), ent_path, timepoint, (instances, colors)).unwrap();
    store.insert(&bundle).unwrap();

    store
}

/// Retrieves a [`DataFrame`] for a [`Component`] with its corresponding
/// [`Instance`] values.
/// ```
/// # use re_arrow_store::{TimelineQuery, TimeQuery};
/// # use re_log_types::{Timeline, field_types::Point2D, msg_bundle::Component};
/// # let store = re_query::__populate_example_store();
///
/// let ent_path = "point";
/// let timeline_query = TimelineQuery::new(
///   Timeline::new_sequence("frame_nr"),
///   TimeQuery::LatestAt(123.into()),
/// );
///
/// let df = re_query::get_component_with_instances(
///   &store,
///   &timeline_query,
///   &ent_path.into(),
///   Point2D::NAME,
/// )
/// .unwrap();
///
/// println!("{:?}", df);
/// ```
///
/// Outputs:
/// ```text
/// ┌──────────┬───────────┐
/// │ instance ┆ point2d   │
/// │ ---      ┆ ---       │
/// │ u64      ┆ struct[2] │
/// ╞══════════╪═══════════╡
/// │ 42       ┆ {1.0,2.0} │
/// ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 96       ┆ {3.0,4.0} │
/// └──────────┴───────────┘
/// ```
///
pub fn get_component_with_instances(
    store: &DataStore,
    timeline_query: &TimelineQuery,
    ent_path: &ObjPath,
    component: ComponentNameRef<'_>,
) -> Result<DataFrame> {
    let components = [Instance::NAME, component];

    let row_indices = store
        .query(timeline_query, ent_path, component, &components)
        .ok_or(QueryError::PrimaryNotFound)?;

    let results = store.get(&components, &row_indices);

    let series: std::result::Result<Vec<Series>, PolarsError> = components
        .iter()
        .zip(results)
        .filter_map(|(component, col)| col.map(|col| (component, col)))
        .map(|(&component, col)| Series::try_from((component, col)))
        .collect();

    let df = DataFrame::new(series?)?;
    let exploded = df.explode(df.get_column_names())?;

    Ok(exploded)
}

/// If a `DataFrame` has no `Instance` column create one from the row numbers
fn add_instances_if_needed(df: DataFrame) -> LazyFrame {
    // Note: we add a row-column temporarily which has u32 type, and then convert it
    // to the correctly named "instance" row with the correct type and drop the original
    // row.
    match df.column(Instance::NAME) {
        Ok(_) => df.lazy(),
        Err(_) => df
            .lazy()
            .with_row_count("tmp_row", None)
            .with_column(col("tmp_row").cast(DataType::UInt64).alias(Instance::NAME))
            .drop_columns(["tmp_row"]),
    }
}

/// Retrieve an entity as a polars Dataframe
///
/// An entity has a primary [`Component`] which is expected to always be
/// present. The length of the batch will be equal to the length of the primary
/// component.
///
/// The remaining components are joined based on their instances. If those not
/// available, they are implicitly treated as an integer sequence of the correct
/// length.
///
/// ```
/// # use re_arrow_store::{TimelineQuery, TimeQuery};
/// # use re_log_types::{Timeline, field_types::{Point2D, ColorRGBA}, msg_bundle::Component};
/// # let store = re_query::__populate_example_store();
///
/// let ent_path = "point";
/// let timeline_query = TimelineQuery::new(
///   Timeline::new_sequence("frame_nr"),
///   TimeQuery::LatestAt(123.into()),
/// );
///
/// let df = re_query::query_entity_with_primary(
///   &store,
///   &timeline_query,
///   &ent_path.into(),
///   Point2D::NAME,
///   &[ColorRGBA::NAME],
/// )
/// .unwrap();
///
/// println!("{:?}", df);
/// ```
///
/// Outputs:
/// ```text
/// ┌──────────┬───────────┬────────────┐
/// │ instance ┆ point2d   ┆ colorrgba  │
/// │ ---      ┆ ---       ┆ ---        │
/// │ u64      ┆ struct[2] ┆ u32        │
/// ╞══════════╪═══════════╪════════════╡
/// │ 42       ┆ {1.0,2.0} ┆ null       │
/// ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 96       ┆ {3.0,4.0} ┆ 4278190080 │
/// └──────────┴───────────┴────────────┘
/// ```
///
pub fn query_entity_with_primary<const N: usize>(
    store: &DataStore,
    timeline_query: &TimelineQuery,
    ent_path: &ObjPath,
    primary: ComponentNameRef<'_>,
    components: &[ComponentNameRef<'_>; N],
) -> Result<DataFrame> {
    let df = get_component_with_instances(store, timeline_query, ent_path, primary)?;

    // TODO(jleibs): lots of room for optimization here. Once "instance" is
    // guaranteed to be sorted we should be able to leverage this during the
    // join. Series have a SetSorted option to specify this. join_asof might be
    // the right place to start digging.

    let ldf = add_instances_if_needed(df);

    let joined = components
        .iter()
        .fold(Ok(ldf), |ldf: Result<LazyFrame>, component| {
            let component =
                get_component_with_instances(store, timeline_query, ent_path, component)?;

            let lazy_component = add_instances_if_needed(component);

            Ok(ldf?.left_join(lazy_component, col(Instance::NAME), col(Instance::NAME)))
        })?
        .collect();

    Ok(joined?)
}

/// Visit all of the components in a dataframe
pub fn visit_components<C1: Component>(df: &DataFrame, mut visit: impl FnMut(Option<&C1>))
where
    C1: ArrowDeserialize + ArrowField<Type = C1> + 'static,
    for<'b> &'b <C1 as ArrowDeserialize>::ArrayType: IntoIterator,
{
    if let Ok(col) = df.column(C1::name()) {
        for chunk_idx in 0..col.n_chunks() {
            // TODO(jleibs): This is an ugly work-around but gets our serializers working again
            // Explanation:
            // Polars Series appear make all fields nullable. Polars doesn't even have a way to
            // express non-nullable types.
            // However, our `field_types` `Component` definitions have non-nullable fields.
            // This causes a "Data type mismatch" on the deserialization and keeps us from
            // getting at our data.
            let col_arrow = col
                .array_ref(chunk_idx)
                .as_any()
                .downcast_ref::<StructArray>()
                .unwrap();
            let component_typed_col =
                StructArray::new(C1::data_type(), col_arrow.clone().into_data().1, None);

            if let Ok(iterable) = arrow_array_deserialize_iterator::<C1>(&component_typed_col) {
                iterable.for_each(|data| visit(Some(&data)));
            };
        }
    }
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
fn component_with_instances() {
    use re_arrow_store::{TimeQuery, TimelineQuery};
    use re_log_types::{field_types::Point2D, msg_bundle::Component, Timeline};

    let store = __populate_example_store();

    let ent_path = "point";
    let timeline_query = TimelineQuery::new(
        Timeline::new_sequence("frame_nr"),
        TimeQuery::LatestAt(123.into()),
    );

    let df = get_component_with_instances(&store, &timeline_query, &ent_path.into(), Point2D::NAME)
        .unwrap();

    eprintln!("{:?}", df);
}

#[test]
fn simple_query() {
    use re_arrow_store::TimeQuery;
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

    // Create some points with implicit instances
    let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];
    let bundle = try_build_msg_bundle1(MsgId::random(), ent_path, timepoint, &points).unwrap();
    store.insert(&bundle).unwrap();

    // Assign one of them a color with an explicit instance
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
    let timeline_query = re_arrow_store::TimelineQuery::new(
        timepoint[0].0,
        TimeQuery::LatestAt(timepoint[0].1.as_i64()),
    );

    let df = query_entity_with_primary(
        &store,
        &timeline_query,
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
    use re_arrow_store::TimeQuery;
    use re_log_types::{
        datagen::build_frame_nr,
        field_types::{ColorRGBA, Instance, Point2D},
        msg_bundle::try_build_msg_bundle1,
        MsgId,
    };

    let mut store = DataStore::default();

    let ent_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Create some points with an implicit instance
    let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];
    let bundle = try_build_msg_bundle1(MsgId::random(), ent_path, timepoint, &points).unwrap();
    store.insert(&bundle).unwrap();

    // Assign them colors with explicit instances
    let colors = vec![ColorRGBA(0xff000000), ColorRGBA(0x00ff0000)];
    let bundle = try_build_msg_bundle1(MsgId::random(), ent_path, timepoint, &colors).unwrap();
    store.insert(&bundle).unwrap();

    // Retrieve the view
    let timeline_query = re_arrow_store::TimelineQuery::new(
        timepoint[0].0,
        TimeQuery::LatestAt(timepoint[0].1.as_i64()),
    );

    let df = query_entity_with_primary(
        &store,
        &timeline_query,
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
