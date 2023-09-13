use std::collections::BTreeMap;

use re_arrow_store::{DataStore, LatestAtQuery};
use re_log_types::{DataRow, EntityPath, LegacyComponent, RowId};
use re_types::{components::InstanceKey, Archetype, Component, ComponentName, Loggable};

use crate::{ArchetypeView, ComponentWithInstances, EntityView, QueryError};

/// Retrieves a [`ComponentWithInstances`] from the [`DataStore`].
///
/// Returns `None` if the component is not found.
///
/// ```
/// # use re_arrow_store::LatestAtQuery;
/// # use re_types::components::Point2D;
/// # use re_log_types::Timeline;
/// # use re_types::Loggable as _;
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
) -> Option<(RowId, ComponentWithInstances)> {
    debug_assert_eq!(store.cluster_key(), InstanceKey::name());

    let components = [InstanceKey::name(), component];

    let (row_id, mut cells) = store.latest_at(query, ent_path, component, &components)?;

    Some((
        row_id,
        ComponentWithInstances {
            // NOTE: The unwrap cannot fail, the cluster key's presence is guaranteed
            // by the store.
            instance_keys: cells[0].take().unwrap(),
            values: cells[1].take()?,
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
/// # use re_types::components::{Point2D, Color};
/// # use re_log_types::Timeline;
/// # use re_types::Loggable as _;
/// # let store = re_query::__populate_example_store();
///
/// let ent_path = "point";
/// let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());
///
/// let entity_view = re_query::query_entity_with_primary::<Point2D>(
///   &store,
///   &query,
///   &ent_path.into(),
///   &[Color::name()],
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
pub fn query_entity_with_primary<Primary: LegacyComponent + Component>(
    store: &DataStore,
    query: &LatestAtQuery,
    ent_path: &EntityPath,
    components: &[ComponentName],
) -> crate::Result<EntityView<Primary>> {
    re_tracing::profile_function!();

    let (row_id, primary) = get_component_with_instances(store, query, ent_path, Primary::name())
        .ok_or(QueryError::PrimaryNotFound(Primary::name()))?;

    // TODO(jleibs): lots of room for optimization here. Once "instance" is
    // guaranteed to be sorted we should be able to leverage this during the
    // join. Series have a SetSorted option to specify this. join_asof might be
    // the right place to start digging.

    let components: BTreeMap<ComponentName, ComponentWithInstances> = components
        .iter()
        // Filter out `Primary` and `InstanceKey` from the component list since they are
        // always queried above when creating the primary.
        .filter(|component| *component != &Primary::name() && *component != &InstanceKey::name())
        .filter_map(|component| {
            get_component_with_instances(store, query, ent_path, *component)
                .map(|(_, component_result)| (*component, component_result))
        })
        .collect();

    Ok(EntityView {
        primary_row_id: row_id,
        primary,
        components,
        phantom: std::marker::PhantomData,
    })
}

/// Retrieve an [`ArchetypeView`] from the `DataStore`
///
/// If you expect only one instance (e.g. for mono-components like `Transform` `Tensor`]
/// and have no additional components you can use [`DataStore::query_latest_component`] instead.
///
/// ```
/// # use re_arrow_store::LatestAtQuery;
/// # use re_log_types::Timeline;
/// # use re_types::Component;
/// # use re_types::components::{Point2D, Color};
/// # use re_types::archetypes::Points2D;
/// # let store = re_query::__populate_example_store();
///
/// let ent_path = "point";
/// let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());
///
/// let arch_view = re_query::query_archetype::<Points2D>(
///   &store,
///   &query,
///   &ent_path.into(),
/// )
/// .unwrap();
///
/// # #[cfg(feature = "polars")]
/// let df = arch_view.as_df2::<Point2D, Color>().unwrap();
///
/// //println!("{df:?}");
/// ```
///
/// Outputs:
/// ```text
/// ┌────────────────────┬───────────────┬─────────────────┐
/// │ rerun.components.InstanceKey ┆ rerun.components.Point2D ┆ rerun.components.Color │
/// │ ---                ┆ ---           ┆ ---             │
/// │ u64                ┆ struct[2]     ┆ u32             │
/// ╞════════════════════╪═══════════════╪═════════════════╡
/// │ 42                 ┆ {1.0,2.0}     ┆ null            │
/// │ 96                 ┆ {3.0,4.0}     ┆ 4278190080      │
/// └────────────────────┴───────────────┴─────────────────┘
/// ```
///
pub fn query_archetype<A: Archetype>(
    store: &DataStore,
    query: &LatestAtQuery,
    ent_path: &EntityPath,
) -> crate::Result<ArchetypeView<A>> {
    re_tracing::profile_function!();

    let required_components: Vec<_> = A::required_components()
        .iter()
        .map(|component| {
            get_component_with_instances(store, query, ent_path, *component)
                .map(|(row_id, component_result)| (row_id, component_result))
        })
        .collect();

    // NOTE: It's important to use `PrimaryNotFound` here. Any other error will be
    // reported to the user.
    //
    // `query_archetype` is currently run for every archetype on every path in the view
    // each path that's missing the primary is then ignored rather than being visited.
    for (name, c) in itertools::izip!(A::required_components().iter(), &required_components) {
        if c.is_none() {
            return crate::Result::Err(QueryError::PrimaryNotFound(*name));
        }
    }

    let (row_ids, required_components): (Vec<_>, Vec<_>) =
        required_components.into_iter().flatten().unzip();

    let row_id = row_ids.first().unwrap_or(&RowId::ZERO);

    let recommended_components = A::recommended_components();
    let optional_components = A::optional_components();

    let other_components = recommended_components
        .iter()
        .chain(optional_components.iter())
        .filter_map(|component| {
            get_component_with_instances(store, query, ent_path, *component)
                .map(|(_, component_result)| component_result)
        });

    Ok(ArchetypeView::from_components(
        *row_id,
        required_components.into_iter().chain(other_components),
    ))
}

/// Helper used to create an example store we can use for querying in doctests
pub fn __populate_example_store() -> DataStore {
    use re_components::datagen::build_frame_nr;
    use re_types::components::{Color, Point2D};

    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    let instances = vec![InstanceKey(42), InstanceKey(96)];
    let points = vec![Point2D::new(1.0, 2.0), Point2D::new(3.0, 4.0)];

    let row = DataRow::from_cells2_sized(
        RowId::random(),
        ent_path,
        timepoint,
        instances.len() as _,
        (&instances, &points),
    );
    store.insert_row(&row).unwrap();

    let instances = vec![InstanceKey(96)];
    let colors = vec![Color::from(0xff000000)];

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
    use re_log_types::Timeline;
    use re_types::components::Point2D;

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
        let points = vec![Some(Point2D::new(1.0, 2.0)), Some(Point2D::new(3.0, 4.0))];

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
    use re_log_types::Timeline;
    use re_types::components::{Color, Point2D};

    let store = __populate_example_store();

    let ent_path = "point";
    let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());

    let entity_view =
        query_entity_with_primary::<Point2D>(&store, &query, &ent_path.into(), &[Color::name()])
            .unwrap();

    #[cfg(feature = "polars")]
    {
        let df = entity_view.as_df2::<Color>().unwrap();
        eprintln!("{df:?}");

        let instances = vec![Some(InstanceKey(42)), Some(InstanceKey(96))];
        let points = vec![Some(Point2D::new(1.0, 2.0)), Some(Point2D::new(3.0, 4.0))];
        let colors = vec![None, Some(Color::from(0xff000000))];

        let expected = crate::dataframe_util::df_builder3(&instances, &points, &colors).unwrap();
        assert_eq!(expected, df);
    }
    #[cfg(not(feature = "polars"))]
    {
        let _used = entity_view;
    }
}

// Minimal test matching the doctest for `query_entity_with_primary`
#[test]
fn simple_query_archetype() {
    use re_arrow_store::LatestAtQuery;
    use re_log_types::Timeline;
    use re_types::archetypes::Points2D;
    use re_types::components::{Color, Point2D};

    let store = __populate_example_store();

    let ent_path = "point";
    let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());

    let arch_view = query_archetype::<Points2D>(&store, &query, &ent_path.into()).unwrap();

    let expected_points = [Point2D::new(1.0, 2.0), Point2D::new(3.0, 4.0)];
    let expected_colors = [None, Some(Color::from_unmultiplied_rgba(255, 0, 0, 0))];

    let view_points: Vec<_> = arch_view
        .iter_required_component::<Point2D>()
        .unwrap()
        .collect();

    let view_colors: Vec<_> = arch_view
        .iter_optional_component::<Color>()
        .unwrap()
        .collect();

    assert_eq!(expected_points, view_points.as_slice());
    assert_eq!(expected_colors, view_colors.as_slice());

    #[cfg(feature = "polars")]
    {
        let df = arch_view.as_df2::<Point2D, Color>().unwrap();
        eprintln!("{df:?}");
    }
}
