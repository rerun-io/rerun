use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::{EntityPath, RowId};
use re_types_core::{components::InstanceKey, Archetype, ComponentName, Loggable};

use crate::{ArchetypeView, ComponentWithInstances, QueryError};

/// Retrieves a [`ComponentWithInstances`] from the [`DataStore`].
///
/// Returns `None` if the component is not found.
///
#[cfg_attr(
    feature = "testing",
    doc = r##"
/// ```
/// # use re_data_store::LatestAtQuery;
/// # use re_log_types::{Timeline, example_components::{MyColor, MyPoint}};
/// # use re_types_core::Loggable as _;
/// # let store = re_query::__populate_example_store();
///
/// let ent_path = "point";
/// let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());
///
/// let (_, component) = re_query::get_component_with_instances(
///   &store,
///   &query,
///   &ent_path.into(),
///   MyPoint::name(),
/// )
/// .unwrap();
///
/// # #[cfg(feature = "polars")]
/// let df = component.as_df::<MyPoint>().unwrap();
///
/// //println!("{df:?}");
/// ```
///
/// Outputs:
/// ```text
/// ┌─────────────┬───────────┐
/// │ InstanceKey ┆ MyPoint   │
/// │ ---         ┆ ---       │
/// │ u64         ┆ struct[2] │
/// ╞═════════════╪═══════════╡
/// │ 42          ┆ {1.0,2.0} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 96          ┆ {3.0,4.0} │
/// └─────────────┴───────────┘
/// ```
"##
)]
pub fn get_component_with_instances(
    store: &DataStore,
    query: &LatestAtQuery,
    ent_path: &EntityPath,
    component: ComponentName,
) -> Option<(RowId, ComponentWithInstances)> {
    debug_assert_eq!(store.cluster_key(), InstanceKey::name());

    let components = [InstanceKey::name(), component];

    let (_, row_id, mut cells) = store.latest_at(query, ent_path, component, &components)?;

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

/// Retrieve an [`ArchetypeView`] from the `DataStore`
///
/// If you expect only one instance (e.g. for mono-components like `Transform` `Tensor`]
/// and have no additional components you can use [`DataStore::query_latest_component`] instead.
///
///
#[cfg_attr(
    feature = "testing",
    doc = r##"
/// ```
/// # use re_data_store::LatestAtQuery;
/// # use re_log_types::{Timeline, example_components::{MyColor, MyPoint, MyPoints}};
/// # use re_types_core::Component;
/// # let store = re_query::__populate_example_store();
///
/// let ent_path = "point";
/// let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());
///
/// let arch_view = re_query::query_archetype::<MyPoints>(
///   &store,
///   &query,
///   &ent_path.into(),
/// )
/// .unwrap();
///
/// # #[cfg(feature = "polars")]
/// let df = arch_view.as_df2::<MyPoint, MyColor>().unwrap();
///
/// //println!("{df:?}");
/// ```
///
/// Outputs:
/// ```text
/// ┌────────────────────┬───────────────┬─────────────────┐
/// │ InstanceKey        ┆ MyPoint       ┆ MyColor         │
/// │ ---                ┆ ---           ┆ ---             │
/// │ u64                ┆ struct[2]     ┆ u32             │
/// ╞════════════════════╪═══════════════╪═════════════════╡
/// │ 42                 ┆ {1.0,2.0}     ┆ null            │
/// │ 96                 ┆ {3.0,4.0}     ┆ 4278190080      │
/// └────────────────────┴───────────────┴─────────────────┘
/// ```
"##
)]
pub fn query_archetype<A: Archetype>(
    store: &DataStore,
    query: &LatestAtQuery,
    ent_path: &EntityPath,
) -> crate::Result<ArchetypeView<A>> {
    re_tracing::profile_function!();

    let required_components: Vec<_> = A::required_components()
        .iter()
        .map(|component| get_component_with_instances(store, query, ent_path, *component))
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
#[cfg(feature = "testing")]
pub fn __populate_example_store() -> DataStore {
    use re_log_types::example_components::{MyColor, MyPoint};
    use re_log_types::{build_frame_nr, DataRow};

    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    let instances = vec![InstanceKey(42), InstanceKey(96)];
    let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];

    let row = DataRow::from_cells2_sized(
        RowId::new(),
        ent_path,
        timepoint,
        instances.len() as _,
        (&instances, &positions),
    )
    .unwrap();
    store.insert_row(&row).unwrap();

    let instances = vec![InstanceKey(96)];
    let colors = vec![MyColor::from(0xff000000)];

    let row = DataRow::from_cells2_sized(
        RowId::new(),
        ent_path,
        timepoint,
        instances.len() as _,
        (instances, colors),
    )
    .unwrap();
    store.insert_row(&row).unwrap();

    store
}

// Minimal test matching the doctest for `get_component_with_instances`
#[test]
#[cfg(test)]
#[cfg(feature = "testing")]
fn simple_get_component() {
    use re_data_store::LatestAtQuery;
    use re_log_types::example_components::MyPoint;
    use re_log_types::Timeline;

    let store = __populate_example_store();

    let ent_path = "point";
    let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());

    let (_, component) =
        get_component_with_instances(&store, &query, &ent_path.into(), MyPoint::name()).unwrap();

    #[cfg(feature = "polars")]
    {
        let df = component.as_df::<MyPoint>().unwrap();
        eprintln!("{df:?}");

        let instances = vec![Some(InstanceKey(42)), Some(InstanceKey(96))];
        let positions = vec![Some(MyPoint::new(1.0, 2.0)), Some(MyPoint::new(3.0, 4.0))];

        let expected = crate::dataframe_util::df_builder2(&instances, &positions).unwrap();

        assert_eq!(expected, df);
    }
    #[cfg(not(feature = "polars"))]
    {
        let _used = component;
    }
}

// Minimal test matching the doctest for `query_entity_with_primary`
#[test]
#[cfg(test)]
#[cfg(feature = "testing")]
fn simple_query_archetype() {
    use re_data_store::LatestAtQuery;
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_log_types::Timeline;

    let store = __populate_example_store();

    let ent_path = "point";
    let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), 123.into());

    let arch_view = query_archetype::<MyPoints>(&store, &query, &ent_path.into()).unwrap();

    let expected_positions = [MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let expected_colors = [None, Some(MyColor::from(0xff000000))];

    let view_positions: Vec<_> = arch_view
        .iter_required_component::<MyPoint>()
        .unwrap()
        .collect();

    let view_colors: Vec<_> = arch_view
        .iter_optional_component::<MyColor>()
        .unwrap()
        .collect();

    assert_eq!(expected_positions, view_positions.as_slice());
    assert_eq!(expected_colors, view_colors.as_slice());

    #[cfg(feature = "polars")]
    {
        let df = arch_view.as_df2::<MyPoint, MyColor>().unwrap();
        eprintln!("{df:?}");
    }
}
