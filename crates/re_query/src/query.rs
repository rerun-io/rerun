use arrow2::array::Array;
use polars_core::{prelude::*, series::IsSorted};
use re_arrow_store::{DataStore, TimelineQuery};
use re_log_types::{field_types::Instance, msg_bundle::Component, ComponentName, ObjPath};

#[derive(thiserror::Error, Debug)]
pub enum QueryError {
    #[error("Tried to access a column that doesn't exist")]
    BadAccess,

    #[error("Could not find primary")]
    PrimaryNotFound,

    #[error("Error executing Polars Query")]
    PolarsError(#[from] PolarsError),
}

pub type Result<T> = std::result::Result<T, QueryError>;

/// A type-erased array of [`Component`] values and the corresponding [`Instance`] keys.
///
/// `instance_keys` must always be sorted if present. If not present we assume implicit
/// instance keys that are equal to the row-number.
///
/// This type can be easily converted into a polars [`DataFrame`]
/// See: [`get_component_with_instances`]
#[derive(Clone, Debug)]
pub struct ComponentWithInstances {
    pub name: ComponentName,
    pub instance_keys: Option<Box<dyn Array>>,
    pub values: Box<dyn Array>,
}

impl TryFrom<ComponentWithInstances> for DataFrame {
    type Error = QueryError;

    fn try_from(val: ComponentWithInstances) -> Result<DataFrame> {
        let mut instance_series = if let Some(instance_keys) = val.instance_keys {
            Series::try_from((Instance::name().as_str(), instance_keys.clone()))?
        } else {
            Series::new(Instance::name().as_str(), 0..val.values.len() as u64)
        };

        // Annotate the instance_series as being sorted.
        // TODO(jleibs): Figure out if dataframe actually makes use of this?
        instance_series.set_sorted(IsSorted::Ascending);

        let value_series = Series::try_from((val.name.as_str(), val.values))?;

        DataFrame::new(vec![instance_series, value_series]).map_err(Into::into)
    }
}

/// Retrieves a [`ComponentWithInstances`] from the [`DataStore`].
/// ```
/// # use polars_core::prelude::DataFrame;
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
/// let df : DataFrame = re_query::get_component_with_instances(
///   &store,
///   &timeline_query,
///   &ent_path.into(),
///   Point2D::name(),
/// )
/// .unwrap().try_into().unwrap();
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
    component: ComponentName,
) -> Result<ComponentWithInstances> {
    let components = [Instance::name(), component];

    let row_indices = store
        .query(timeline_query, ent_path, component, &components)
        .ok_or(QueryError::PrimaryNotFound)?;

    let mut results = store.get(&components, &row_indices);

    Ok(ComponentWithInstances {
        name: component,
        instance_keys: results[0].take(),
        values: results[1].take().ok_or(QueryError::PrimaryNotFound)?,
    })
}

#[derive(Clone, Debug)]
pub struct EntityView {
    pub primary: ComponentWithInstances,
    pub components: Vec<ComponentWithInstances>,
}

impl TryFrom<EntityView> for DataFrame {
    type Error = QueryError;

    fn try_from(val: EntityView) -> Result<DataFrame> {
        let df: DataFrame = val.primary.try_into()?;

        // TODO(jleibs): lots of room for optimization here. Once "instance" is
        // guaranteed to be sorted we should be able to leverage this during the
        // join. Series have a SetSorted option to specify this. join_asof might be
        // the right place to start digging.

        val.components
            .into_iter()
            .fold(Ok(df), |df: Result<DataFrame>, component| {
                let component_df: DataFrame = component.try_into()?;
                // We use an asof join which takes advantage of the fact
                // that our join-columns are sorted. The strategy shouldn't
                // matter here since we have a Tolerance of None.
                let joined = df?.join_asof(
                    &component_df,
                    Instance::name().as_str(),
                    Instance::name().as_str(),
                    AsofStrategy::Backward,
                    None,
                    None,
                );
                Ok(joined?)
            })
    }
}

/// Retrieve an `EntityView` from the `DataStore`
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
/// # use polars_core::prelude::DataFrame;
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
/// let df : DataFrame = re_query::query_entity_with_primary(
///   &store,
///   &timeline_query,
///   &ent_path.into(),
///   Point2D::name(),
///   &[ColorRGBA::name()],
/// )
/// .unwrap().try_into().unwrap();
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
    primary: ComponentName,
    components: &[ComponentName; N],
) -> Result<EntityView> {
    let primary = get_component_with_instances(store, timeline_query, ent_path, primary)?;

    // TODO(jleibs): lots of room for optimization here. Once "instance" is
    // guaranteed to be sorted we should be able to leverage this during the
    // join. Series have a SetSorted option to specify this. join_asof might be
    // the right place to start digging.

    let components: Result<Vec<ComponentWithInstances>> = components
        .iter()
        .filter_map(|component| {
            match get_component_with_instances(store, timeline_query, ent_path, *component) {
                Ok(component_result) => Some(Ok(component_result)),
                Err(QueryError::PrimaryNotFound) => None,
                Err(err) => Some(Err(err)),
            }
        })
        .collect();

    Ok(EntityView {
        primary,
        components: components?,
    })
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
    use re_log_types::{field_types::Point2D, msg_bundle::Component as _, Timeline};

    let store = __populate_example_store();

    let ent_path = "point";
    let timeline_query = TimelineQuery::new(
        Timeline::new_sequence("frame_nr"),
        TimeQuery::LatestAt(123.into()),
    );

    let df =
        get_component_with_instances(&store, &timeline_query, &ent_path.into(), Point2D::name())
            .unwrap();
    eprintln!("{:?}", df);

    let instances = vec![Some(Instance(42)), Some(Instance(96))];
    let points = vec![
        Some(Point2D { x: 1.0, y: 2.0 }),
        Some(Point2D { x: 3.0, y: 4.0 }),
    ];

    let expected = df_builder2(&instances, &points).unwrap();

    assert_eq!(expected, df.try_into().unwrap());
}
