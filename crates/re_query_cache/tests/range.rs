//! Contains:
//! - A 1:1 port of the tests in `crates/re_query/tests/archetype_range_tests.rs`, with caching enabled.
//! - Invalidation tests.

use itertools::Itertools as _;

use re_data_store::{DataStore, RangeQuery};
use re_log_types::{
    build_frame_nr,
    example_components::{MyColor, MyLabel, MyPoint, MyPoints},
    DataRow, EntityPath, RowId, TimeInt, TimeRange,
};
use re_query_cache::query_archetype_pov1_comp2;
use re_types::components::InstanceKey;
use re_types_core::Loggable as _;

// ---

#[test]
fn simple_range() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123.into())];
    {
        // Create some Positions with implicit instances
        let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), timepoint1, 2, positions)
                .unwrap();
        store.insert_row(&row).unwrap();

        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(1)];
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint1,
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint2 = [build_frame_nr(223.into())];
    {
        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(0)];
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint2,
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint3 = [build_frame_nr(323.into())];
    {
        // Create some Positions with implicit instances
        let positions = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), timepoint3, 2, positions)
                .unwrap();
        store.insert_row(&row).unwrap();
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    );

    query_and_compare(&store, &query, &ent_path);

    // --- Second test: `[timepoint1, timepoint3]` ---

    // The inclusion of `timepoint1` means latest-at semantics will _not_ kick in!

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    query_and_compare(&store, &query, &ent_path);
}

#[test]
fn timeless_range() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123.into())];
    {
        // Create some Positions with implicit instances
        let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let mut row =
            DataRow::from_cells1(RowId::new(), ent_path.clone(), timepoint1, 2, &positions)
                .unwrap();
        row.compute_all_size_bytes();
        store.insert_row(&row).unwrap();

        // Insert timelessly too!
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), [], 2, &positions).unwrap();
        store.insert_row(&row).unwrap();

        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(1)];
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint1,
            1,
            (color_instances.clone(), colors.clone()),
        )
        .unwrap();
        store.insert_row(&row).unwrap();

        // Insert timelessly too!
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            [],
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint2 = [build_frame_nr(223.into())];
    {
        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(0)];
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint2,
            1,
            (color_instances.clone(), colors.clone()),
        )
        .unwrap();
        store.insert_row(&row).unwrap();

        // Insert timelessly too!
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint2,
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint3 = [build_frame_nr(323.into())];
    {
        // Create some Positions with implicit instances
        let positions = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), timepoint3, 2, &positions)
                .unwrap();
        store.insert_row(&row).unwrap();

        // Insert timelessly too!
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), [], 2, &positions).unwrap();
        store.insert_row(&row).unwrap();
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    );

    query_and_compare(&store, &query, &ent_path);

    // --- Second test: `[timepoint1, timepoint3]` ---

    // The inclusion of `timepoint1` means latest-at semantics will fall back to timeless data!

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    query_and_compare(&store, &query, &ent_path);

    // --- Third test: `[-inf, +inf]` ---

    eprintln!("XXXXXXXXXXXXXXXXXXXXXXXX");

    let query =
        re_data_store::RangeQuery::new(timepoint1[0].0, TimeRange::new(TimeInt::MIN, TimeInt::MAX));

    query_and_compare(&store, &query, &ent_path);
}

#[test]
fn simple_splatted_range() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123.into())];
    {
        // Create some Positions with implicit instances
        let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), timepoint1, 2, positions)
                .unwrap();
        store.insert_row(&row).unwrap();

        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(1)];
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint1,
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint2 = [build_frame_nr(223.into())];
    {
        // Assign one of them a color with a splatted instance
        let color_instances = vec![InstanceKey::SPLAT];
        let colors = vec![MyColor::from_rgb(0, 255, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint2,
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint3 = [build_frame_nr(323.into())];
    {
        // Create some Positions with implicit instances
        let positions = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), timepoint3, 2, positions)
                .unwrap();
        store.insert_row(&row).unwrap();
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    );

    query_and_compare(&store, &query, &ent_path);

    // --- Second test: `[timepoint1, timepoint3]` ---

    // The inclusion of `timepoint1` means latest-at semantics will _not_ kick in!

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    query_and_compare(&store, &query, &ent_path);
}

// ---

fn query_and_compare(store: &DataStore, query: &RangeQuery, ent_path: &EntityPath) {
    for _ in 0..3 {
        let mut uncached_data_times = Vec::new();
        let mut uncached_instance_keys = Vec::new();
        let mut uncached_positions = Vec::new();
        let mut uncached_colors = Vec::new();
        query_archetype_pov1_comp2::<MyPoints, MyPoint, MyColor, MyLabel, _>(
            false, // cached?
            store,
            &query.clone().into(),
            ent_path,
            |((data_time, _), instance_keys, positions, colors, _)| {
                uncached_data_times.push(data_time);
                uncached_instance_keys.push(instance_keys.to_vec());
                uncached_positions.push(positions.to_vec());
                uncached_colors.push(colors.to_vec());
            },
        )
        .unwrap();

        let mut cached_data_times = Vec::new();
        let mut cached_instance_keys = Vec::new();
        let mut cached_positions = Vec::new();
        let mut cached_colors = Vec::new();
        query_archetype_pov1_comp2::<MyPoints, MyPoint, MyColor, MyLabel, _>(
            true, // cached?
            store,
            &query.clone().into(),
            ent_path,
            |((data_time, _), instance_keys, positions, colors, _)| {
                cached_data_times.push(data_time);
                cached_instance_keys.push(instance_keys.to_vec());
                cached_positions.push(positions.to_vec());
                cached_colors.push(colors.to_vec());
            },
        )
        .unwrap();

        let mut expected_data_times = Vec::new();
        let mut expected_instance_keys = Vec::new();
        let mut expected_positions = Vec::new();
        let mut expected_colors = Vec::new();
        let expected = re_query::range_archetype::<MyPoints, { MyPoints::NUM_COMPONENTS }>(
            store, query, ent_path,
        );
        for arch_view in expected {
            expected_data_times.push(arch_view.data_time());
            expected_instance_keys.push(arch_view.iter_instance_keys().collect_vec());
            expected_positions.push(
                arch_view
                    .iter_required_component::<MyPoint>()
                    .unwrap()
                    .collect_vec(),
            );
            expected_colors.push(
                arch_view
                    .iter_optional_component::<MyColor>()
                    .unwrap()
                    .collect_vec(),
            );
        }

        // Keep this around for the next unlucky chap.
        eprintln!("(expected={expected_data_times:?}, uncached={uncached_data_times:?}, cached={cached_data_times:?})");
        // eprintln!("{}", store.to_data_table().unwrap());

        similar_asserts::assert_eq!(expected_data_times, uncached_data_times);
        similar_asserts::assert_eq!(expected_instance_keys, uncached_instance_keys);
        similar_asserts::assert_eq!(expected_positions, uncached_positions);
        similar_asserts::assert_eq!(expected_colors, uncached_colors);

        similar_asserts::assert_eq!(expected_data_times, cached_data_times);
        similar_asserts::assert_eq!(expected_instance_keys, cached_instance_keys);
        similar_asserts::assert_eq!(expected_positions, cached_positions);
        similar_asserts::assert_eq!(expected_colors, cached_colors);
    }
}
