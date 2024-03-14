use itertools::Itertools;

use re_data_store::{test_row, DataStore};
use re_log_types::EntityPath;
use re_log_types::{build_frame_nr, TimeInt, TimeType, Timeline};
use re_types::{
    archetypes::Points2D,
    components::{Color, InstanceKey, Position2D},
    datagen::{build_some_colors, build_some_positions2d},
};
use re_types_core::Loggable as _;

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
    let row = test_row!(ent_path @ [build_frame_nr(42.try_into().unwrap())] => 3; [positions.clone(), colors.clone()]);
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
