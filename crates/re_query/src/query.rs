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
            // If we find the component, then we try to left-join with the existing dataframe
            // If the column we are looking up isn't found, just return the dataframe as is
            // For any other error, escalate
            match get_component_with_instances(store, timeline_query, ent_path, component) {
                Ok(component_df) => {
                    let lazy_component = add_instances_if_needed(component_df);
                    Ok(ldf?.left_join(lazy_component, col(Instance::NAME), col(Instance::NAME)))
                }
                Err(QueryError::PrimaryNotFound) => ldf,
                Err(err) => Err(err),
            }
        })?
        .collect();

    Ok(joined?)
}

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
        try_build_msg_bundle2(MsgId::ZERO, ent_path, timepoint, (&instances, &points)).unwrap();
    store.insert(&bundle).unwrap();

    let instances = vec![Instance(96)];
    let colors = vec![ColorRGBA(0xff000000)];
    let bundle =
        try_build_msg_bundle2(MsgId::ZERO, ent_path, timepoint, (instances, colors)).unwrap();
    store.insert(&bundle).unwrap();

    store
}

#[test]
fn component_with_instances() {
    use crate::dataframe_util::df_builder2;
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
    //eprintln!("{:?}", df);

    let instances = vec![Some(Instance(42)), Some(Instance(96))];
    let points = vec![
        Some(Point2D { x: 1.0, y: 2.0 }),
        Some(Point2D { x: 3.0, y: 4.0 }),
    ];

    let expected = df_builder2(&instances, &points).unwrap();

    assert_eq!(df, expected);
}
