use std::collections::BTreeMap;

use re_arrow_store::{DataStore, LatestAtQuery};
use re_log_types::{
    component_types::InstanceKey, Component, ComponentName, DataRow, EntityPath, RowId,
};

use crate::{ComponentWithInstances, EntityView, QueryError};

/// Retrieves a [`ComponentWithInstances`] from the [`DataStore`].
/// ```
/// # use re_arrow_store::LatestAtQuery;
/// # use re_log_types::{Timeline, component_types::Point2D, Component};
/// # let store = re_query::__populate_example_store();
///
/// let ent_path = "point";
/// let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());
///
/// let (_, component) = re_query::get_component_with_instances(
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
/// //println!("{df:?}");
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
    ent_path: &EntityPath,
    component: ComponentName,
) -> crate::Result<(RowId, ComponentWithInstances)> {
    debug_assert_eq!(store.cluster_key(), InstanceKey::name());

    let components = [InstanceKey::name(), component];

    let (row_id, mut cells) = store
        .latest_at(query, ent_path, component, &components)
        .ok_or(QueryError::PrimaryNotFound)?;

    Ok((
        row_id,
        ComponentWithInstances {
            // NOTE: The unwrap cannot fail, the cluster key's presence is guaranteed
            // by the store.
            instance_keys: cells[0].take().unwrap(),
            values: cells[1].take().ok_or(QueryError::PrimaryNotFound)?,
        },
    ))
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
/// If you expect only one instance (e.g. for mono-components like `Transform` `Tensor`]
/// and have no additional components you can use [`DataStore::query_latest_component`] instead.
///
/// ```
/// # use re_arrow_store::LatestAtQuery;
/// # use re_log_types::{Timeline, component_types::{Point2D, ColorRGBA}, Component};
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
/// //println!("{df:?}");
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
    ent_path: &EntityPath,
    components: &[ComponentName],
) -> crate::Result<EntityView<Primary>> {
    crate::profile_function!();

    let (row_id, primary) = get_component_with_instances(store, query, ent_path, Primary::name())?;

    // TODO(jleibs): lots of room for optimization here. Once "instance" is
    // guaranteed to be sorted we should be able to leverage this during the
    // join. Series have a SetSorted option to specify this. join_asof might be
    // the right place to start digging.

    let components: crate::Result<BTreeMap<ComponentName, ComponentWithInstances>> = components
        .iter()
        // Filter out `Primary` and `InstanceKey` from the component list since they are
        // always queried above when creating the primary.
        .filter(|component| *component != &Primary::name() && *component != &InstanceKey::name())
        .filter_map(|component| {
            match get_component_with_instances(store, query, ent_path, *component)
                .map(|(_, cwi)| cwi)
            {
                Ok(component_result) => Some(Ok((*component, component_result))),
                Err(QueryError::PrimaryNotFound) => None,
                Err(err) => Some(Err(err)),
            }
        })
        .collect();

    Ok(EntityView {
        row_id,
        primary,
        components: components?,
        phantom: std::marker::PhantomData,
    })
}

/// Helper used to create an example store we can use for querying in doctests
pub fn __populate_example_store() -> DataStore {
    use re_log_types::{
        component_types::{ColorRGBA, Point2D},
        datagen::build_frame_nr,
    };

    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    let instances = vec![InstanceKey(42), InstanceKey(96)];
    let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];

    let row = DataRow::from_cells2_sized(
        RowId::random(),
        ent_path,
        timepoint,
        instances.len() as _,
        (&instances, &points),
    );
    store.insert_row(&row).unwrap();

    let instances = vec![InstanceKey(96)];
    let colors = vec![ColorRGBA(0xff000000)];

    let row = DataRow::from_cells2_sized(
        RowId::random(),
        ent_path,
        timepoint,
        instances.len() as _,
        (instances, colors),
    );
    store.insert_row(&row).unwrap();

    store
}

// Minimal test matching the doctest for `get_component_with_instances`
#[test]
fn simple_get_component() {
    use re_arrow_store::LatestAtQuery;
    use re_log_types::{component_types::Point2D, Component as _, Timeline};

    let store = __populate_example_store();

    let ent_path = "point";
    let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());

    let (_, component) =
        get_component_with_instances(&store, &query, &ent_path.into(), Point2D::name()).unwrap();

    #[cfg(feature = "polars")]
    {
        let df = component.as_df::<Point2D>().unwrap();
        eprintln!("{df:?}");

        let instances = vec![Some(InstanceKey(42)), Some(InstanceKey(96))];
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
        component_types::{ColorRGBA, Point2D},
        Component as _, Timeline,
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
        eprintln!("{df:?}");

        let instances = vec![Some(InstanceKey(42)), Some(InstanceKey(96))];
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
