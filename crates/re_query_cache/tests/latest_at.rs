//! Contains:
//! - A 1:1 port of the tests in `crates/re_query/tests/archetype_query_tests.rs`, with caching enabled.
//! - Invalidation tests.

use itertools::Itertools as _;

use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::{build_frame_nr, DataRow, EntityPath, RowId};
use re_query_cache::query_archetype_pov1_comp1;
use re_types::{
    archetypes::Points2D,
    components::{Color, InstanceKey, Position2D},
};
use re_types_core::Loggable as _;

// ---

#[test]
fn simple_query() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    // Create some positions with implicit instances
    let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
    let row = DataRow::from_cells1_sized(RowId::new(), ent_path, timepoint, 2, positions).unwrap();
    store.insert_row(&row).unwrap();

    // Assign one of them a color with an explicit instance
    let color_instances = vec![InstanceKey(1)];
    let colors = vec![Color::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells2_sized(
        RowId::new(),
        ent_path,
        timepoint,
        1,
        (color_instances, colors),
    )
    .unwrap();
    store.insert_row(&row).unwrap();

    // Retrieve the view
    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&store, &query, &ent_path.into());
}

#[test]
fn timeless_query() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    // Create some positions with implicit instances
    let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
    let row = DataRow::from_cells1_sized(RowId::new(), ent_path, timepoint, 2, positions).unwrap();
    store.insert_row(&row).unwrap();

    // Assign one of them a color with an explicit instance.. timelessly!
    let color_instances = vec![InstanceKey(1)];
    let colors = vec![Color::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells2_sized(RowId::new(), ent_path, [], 1, (color_instances, colors))
        .unwrap();
    store.insert_row(&row).unwrap();

    // Retrieve the view
    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&store, &query, &ent_path.into());
}

#[test]
fn no_instance_join_query() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    // Create some positions with an implicit instance
    let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
    let row = DataRow::from_cells1_sized(RowId::new(), ent_path, timepoint, 2, positions).unwrap();
    store.insert_row(&row).unwrap();

    // Assign them colors with explicit instances
    let colors = vec![Color::from_rgb(255, 0, 0), Color::from_rgb(0, 255, 0)];
    let row = DataRow::from_cells1_sized(RowId::new(), ent_path, timepoint, 2, colors).unwrap();
    store.insert_row(&row).unwrap();

    // Retrieve the view
    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&store, &query, &ent_path.into());
}

#[test]
fn missing_column_join_query() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    // Create some positions with an implicit instance
    let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
    let row = DataRow::from_cells1_sized(RowId::new(), ent_path, timepoint, 2, positions).unwrap();
    store.insert_row(&row).unwrap();

    // Retrieve the view
    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&store, &query, &ent_path.into());
}

#[test]
fn splatted_query() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    // Create some positions with implicit instances
    let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
    let row = DataRow::from_cells1_sized(RowId::new(), ent_path, timepoint, 2, positions).unwrap();
    store.insert_row(&row).unwrap();

    // Assign all of them a color via splat
    let color_instances = vec![InstanceKey::SPLAT];
    let colors = vec![Color::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells2_sized(
        RowId::new(),
        ent_path,
        timepoint,
        1,
        (color_instances, colors),
    )
    .unwrap();
    store.insert_row(&row).unwrap();

    // Retrieve the view
    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&store, &query, &ent_path.into());
}

#[test]
// TODO(cmc): implement invalidation + in-depth invalidation tests + in-depth OOO tests
#[should_panic(expected = "assertion failed: `(left == right)`")]
fn invalidation() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    // Create some positions with implicit instances
    let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
    let row = DataRow::from_cells1_sized(RowId::new(), ent_path, timepoint, 2, positions).unwrap();
    store.insert_row(&row).unwrap();

    // Assign one of them a color with an explicit instance
    let color_instances = vec![InstanceKey(1)];
    let colors = vec![Color::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells2_sized(
        RowId::new(),
        ent_path,
        timepoint,
        1,
        (color_instances, colors),
    )
    .unwrap();
    store.insert_row(&row).unwrap();

    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&store, &query, &ent_path.into());

    // Invalidate the PoV component
    let positions = vec![Position2D::new(10.0, 20.0), Position2D::new(30.0, 40.0)];
    let row = DataRow::from_cells1_sized(RowId::new(), ent_path, timepoint, 2, positions).unwrap();
    store.insert_row(&row).unwrap();

    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&store, &query, &ent_path.into());

    // Invalidate the optional component
    let colors = vec![Color::from_rgb(255, 0, 0), Color::from_rgb(0, 255, 0)];
    let row = DataRow::from_cells1_sized(RowId::new(), ent_path, timepoint, 2, colors).unwrap();
    store.insert_row(&row).unwrap();

    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&store, &query, &ent_path.into());
}

// ---

fn query_and_compare(store: &DataStore, query: &LatestAtQuery, ent_path: &EntityPath) {
    for _ in 0..3 {
        let mut got_instance_keys = Vec::new();
        let mut got_positions = Vec::new();
        let mut got_colors = Vec::new();

        query_archetype_pov1_comp1::<Points2D, Position2D, Color, _>(
            true,
            store,
            &query.clone().into(),
            ent_path,
            |(_, instance_keys, positions, colors)| {
                got_instance_keys.extend(instance_keys.iter().copied());
                got_positions.extend(positions.iter().copied());
                got_colors.extend(colors.iter().copied());
            },
        )
        .unwrap();

        let expected = re_query::query_archetype::<Points2D>(store, query, ent_path).unwrap();

        let expected_instance_keys = expected.iter_instance_keys().collect_vec();
        let expected_positions = expected
            .iter_required_component::<Position2D>()
            .unwrap()
            .collect_vec();
        let expected_colors = expected
            .iter_optional_component::<Color>()
            .unwrap()
            .collect_vec();

        similar_asserts::assert_eq!(expected_instance_keys, got_instance_keys);
        similar_asserts::assert_eq!(expected_positions, got_positions);
        similar_asserts::assert_eq!(expected_colors, got_colors);
    }
}
