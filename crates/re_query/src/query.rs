use std::collections::BTreeMap;

use re_arrow_store::{DataStore, LatestAtQuery};
use re_log_types::{field_types::Instance, msg_bundle::Component, ComponentName, ObjPath};

use crate::{ComponentWithInstances, EntityView, QueryError};

/// Retrieves a [`ComponentWithInstances`] from the [`DataStore`].
/// ```
/// # use re_arrow_store::LatestAtQuery;
/// # use re_log_types::{Timeline, field_types::Point2D, msg_bundle::Component};
/// # let store = re_query::__populate_example_store();
///
/// let ent_path = "point";
/// let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());
///
/// let component = re_query::get_component_with_instances(
///   &store,
///   &query,
///   &ent_path.into(),
///   Point2D::name(),
/// )
/// .unwrap();
///
/// # #[cfg(feature = "polars")]
/// let df = component.as_df::<Point2D>().unwrap();
///
/// //println!("{:?}", df);
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
    query: &LatestAtQuery,
    ent_path: &ObjPath,
    component: ComponentName,
) -> crate::Result<ComponentWithInstances> {
    let components = [Instance::name(), component];

    let row_indices = store
        .latest_at(query, ent_path, component, &components)
        .ok_or(QueryError::PrimaryNotFound)?;

    let mut results = store.get(&components, &row_indices);

    Ok(ComponentWithInstances {
        name: component,
        instance_keys: results[0].take(),
        values: results[1].take().ok_or(QueryError::PrimaryNotFound)?,
    })
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
/// # use re_arrow_store::LatestAtQuery;
/// # use re_log_types::{Timeline, field_types::{Point2D, ColorRGBA}, msg_bundle::Component};
/// # let store = re_query::__populate_example_store();
///
/// let ent_path = "point";
/// let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());
///
/// let entity_view = re_query::query_entity_with_primary::<Point2D>(
///   &store,
///   &query,
///   &ent_path.into(),
///   &[ColorRGBA::name()],
/// )
/// .unwrap();
///
/// # #[cfg(feature = "polars")]
/// let df = entity_view.as_df1().unwrap();
///
/// //println!("{:?}", df);
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
pub fn query_entity_with_primary<Primary: Component>(
    store: &DataStore,
    query: &LatestAtQuery,
    ent_path: &ObjPath,
    components: &[ComponentName],
) -> crate::Result<EntityView<Primary>> {
    let primary = get_component_with_instances(store, query, ent_path, Primary::name())?;

    // TODO(jleibs): lots of room for optimization here. Once "instance" is
    // guaranteed to be sorted we should be able to leverage this during the
    // join. Series have a SetSorted option to specify this. join_asof might be
    // the right place to start digging.

    let components: crate::Result<BTreeMap<ComponentName, ComponentWithInstances>> = components
        .iter()
        // Filter out `Primary` and `Instance` from the component list since are
        // always queried above when creating the primary.
        .filter(|component| *component != &Primary::name() && *component != &Instance::name())
        .filter_map(|component| {
            match get_component_with_instances(store, query, ent_path, *component) {
                Ok(component_result) => Some(Ok((*component, component_result))),
                Err(QueryError::PrimaryNotFound) => None,
                Err(err) => Some(Err(err)),
            }
        })
        .collect();

    Ok(EntityView {
        primary,
        components: components?,
        phantom: std::marker::PhantomData,
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

    let mut store = DataStore::new(Instance::name(), Default::default());

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

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

// Minimal test matching the doctest for `get_component_with_instances`
#[test]
fn simple_get_component() {
    use re_arrow_store::LatestAtQuery;
    use re_log_types::{field_types::Point2D, msg_bundle::Component as _, Timeline};

    let store = __populate_example_store();

    let ent_path = "point";
    let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());

    let component =
        get_component_with_instances(&store, &query, &ent_path.into(), Point2D::name()).unwrap();

    #[cfg(feature = "polars")]
    {
        let df = component.as_df::<Point2D>().unwrap();
        eprintln!("{:?}", df);

        let instances = vec![Some(Instance(42)), Some(Instance(96))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];

        let expected = crate::dataframe_util::df_builder2(&instances, &points).unwrap();

        assert_eq!(expected, df);
    }
    #[cfg(not(feature = "polars"))]
    {
        let _used = component;
    }
}

// Minimal test matching the doctest for `query_entity_with_primary`
#[test]
fn simple_query_entity() {
    use re_arrow_store::LatestAtQuery;
    use re_log_types::{
        field_types::{ColorRGBA, Point2D},
        msg_bundle::Component as _,
        Timeline,
    };

    let store = __populate_example_store();

    let ent_path = "point";
    let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());

    let entity_view = query_entity_with_primary::<Point2D>(
        &store,
        &query,
        &ent_path.into(),
        &[ColorRGBA::name()],
    )
    .unwrap();

    #[cfg(feature = "polars")]
    {
        let df = entity_view.as_df2::<ColorRGBA>().unwrap();
        eprintln!("{:?}", df);

        let instances = vec![Some(Instance(42)), Some(Instance(96))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let colors = vec![None, Some(ColorRGBA(0xff000000))];

        let expected = crate::dataframe_util::df_builder3(&instances, &points, &colors).unwrap();
        assert_eq!(expected, df);
    }
    #[cfg(not(feature = "polars"))]
    {
        let _used = entity_view;
    }
}
