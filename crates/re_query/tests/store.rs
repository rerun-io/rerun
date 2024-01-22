use std::collections::HashMap;

use itertools::Itertools;

use re_data_store::{
    test_row,
    test_util::{insert_table_with_retries, sanity_unwrap},
    DataStore, GarbageCollectionOptions, LatestAtQuery,
};
use re_log_types::EntityPath;
use re_log_types::{build_frame_nr, DataRow, DataTable, TableId, TimeInt, TimeType, Timeline};
use re_types::{
    archetypes::Points2D,
    components::{Color, InstanceKey, Position2D},
    datagen::{build_some_colors, build_some_instances, build_some_positions2d},
};
use re_types_core::{Archetype, Component, ComponentName, Loggable as _};

// --- LatestAt ---

// TODO: at this point, why is this even here?

#[test]
fn latest_at() {
    for config in re_data_store::test_util::all_configs() {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
        latest_at_impl(&mut store);
    }
}

fn latest_at_impl(store: &mut DataStore) {
    let ent_path = EntityPath::from("this/that");

    let frame0 = TimeInt::from(0);
    let frame1 = TimeInt::from(1);
    let frame2 = TimeInt::from(2);
    let frame3 = TimeInt::from(3);
    let frame4 = TimeInt::from(4);

    // helper to insert a table both as a temporal and timeless payload
    let insert_table = |store: &mut DataStore, table: &DataTable| {
        // insert temporal
        insert_table_with_retries(store, table);

        // insert timeless
        let mut table_timeless = table.clone();
        table_timeless.col_timelines = Default::default();
        insert_table_with_retries(store, &table_timeless);
    };

    let (instances1, colors1) = (build_some_instances(3), build_some_colors(3));
    let row1 = test_row!(ent_path @ [build_frame_nr(frame1)] => 3; [instances1.clone(), colors1]);

    let positions2 = build_some_positions2d(3);
    let row2 = test_row!(ent_path @ [build_frame_nr(frame2)] => 3; [instances1, positions2]);

    let points3 = build_some_positions2d(10);
    let row3 = test_row!(ent_path @ [build_frame_nr(frame3)] => 10; [points3]);

    let colors4 = build_some_colors(5);
    let row4 = test_row!(ent_path @ [build_frame_nr(frame4)] => 5; [colors4]);

    insert_table(
        store,
        &DataTable::from_rows(
            TableId::new(),
            [row1.clone(), row2.clone(), row3.clone(), row4.clone()],
        ),
    );

    // Stress test save-to-disk & load-from-disk
    let mut store2 = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        store.cluster_key(),
        store.config().clone(),
    );
    for table in store.to_data_tables(None) {
        insert_table(&mut store2, &table);
    }
    // Stress test GC
    store2.gc(&GarbageCollectionOptions::gc_everything());
    for table in store.to_data_tables(None) {
        insert_table(&mut store2, &table);
    }
    let store = store2;

    sanity_unwrap(&store);

    fn assert_latest_components<
        A: Archetype,
        P1: Component + PartialEq + std::fmt::Debug,
        C1: Component + PartialEq + std::fmt::Debug,
    >(
        store: &DataStore,
        entity_path: &EntityPath,
        frame_nr: TimeInt,
        rows: &[(ComponentName, &DataRow)],
    ) {
        let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

        let arch_view = re_query::query_archetype::<A>(
            store,
            &LatestAtQuery::new(timeline_frame_nr, frame_nr),
            entity_path,
        )
        .unwrap();

        let expected_p1 = arch_view
            .iter_required_component::<P1>()
            .unwrap()
            .collect_vec();
        let expected_c1 = arch_view
            .iter_raw_optional_component::<C1>()
            .unwrap()
            .unwrap()
            .collect_vec();

        let rows: HashMap<ComponentName, &DataRow> = rows.iter().copied().collect();

        let actual_p1 = {
            let cells = &rows[&P1::name()].cells;
            cells.last().unwrap().to_native::<P1>()
        };
        let actual_c1 = {
            let cells = &rows[&C1::name()].cells;
            cells.last().unwrap().to_native::<C1>()
        };

        assert_eq!(expected_p1, actual_p1);
        assert_eq!(expected_c1, actual_c1);
    }

    assert_latest_components::<Points2D, Position2D, Color>(
        &store,
        &ent_path,
        frame0,
        &[(Color::name(), &row4), (Position2D::name(), &row3)], // timeless
    );
    assert_latest_components::<Points2D, Position2D, Color>(
        &store,
        &ent_path,
        frame1,
        &[
            (Color::name(), &row1),
            (Position2D::name(), &row3), // timeless
        ],
    );
    assert_latest_components::<Points2D, Position2D, Color>(
        &store,
        &ent_path,
        frame2,
        &[(Color::name(), &row1), (Position2D::name(), &row2)],
    );
    assert_latest_components::<Points2D, Position2D, Color>(
        &store,
        &ent_path,
        frame3,
        &[(Color::name(), &row1), (Position2D::name(), &row3)],
    );
    assert_latest_components::<Points2D, Position2D, Color>(
        &store,
        &ent_path,
        frame4,
        &[(Color::name(), &row4), (Position2D::name(), &row3)],
    );
}

// ---

// This one demonstrates a nasty edge case when stream-joining multiple iterators that happen to
// share the same exact row of data at some point (because, for that specific entry, it turns out
// that those component where inserted together).
//
// When that happens, one must be very careful to not only compare time and index row numbers, but
// also make sure that, if all else if equal, the primary iterator comes last so that it gathers as
// much state as possible!

#[test]
fn range_join_across_single_row() {
    for config in re_data_store::test_util::all_configs() {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
        range_join_across_single_row_impl(&mut store);
    }
}

fn range_join_across_single_row_impl(store: &mut DataStore) {
    let ent_path = EntityPath::from("this/that");

    let positions = build_some_positions2d(3);
    let colors = build_some_colors(3);
    let row =
        test_row!(ent_path @ [build_frame_nr(42.into())] => 3; [positions.clone(), colors.clone()]);
    store.insert_row(&row).unwrap();

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let query = re_data_store::RangeQuery::new(
        timeline_frame_nr,
        re_data_store::TimeRange::new(TimeInt::MIN, TimeInt::MAX),
    );

    let mut arch_views = re_query::range_archetype::<Points2D, { Points2D::NUM_COMPONENTS }>(
        store, &query, &ent_path,
    );

    let arch_view = arch_views.next().unwrap();
    assert!(arch_views.next().is_none());

    // dbg!(arch_view);

    let actual_instance_keys = arch_view.iter_instance_keys().collect_vec();
    let actual_positions = arch_view
        .iter_required_component::<Position2D>()
        .unwrap()
        .collect_vec();
    let actual_colors = arch_view
        .iter_optional_component::<Color>()
        .unwrap()
        .collect_vec();

    let expected_instance_keys = vec![InstanceKey(0), InstanceKey(1), InstanceKey(2)];
    let expected_positions = positions;
    let expected_colors = colors.into_iter().map(Some).collect_vec();

    similar_asserts::assert_eq!(expected_instance_keys, actual_instance_keys);
    similar_asserts::assert_eq!(expected_positions, actual_positions);
    similar_asserts::assert_eq!(expected_colors, actual_colors);
}
