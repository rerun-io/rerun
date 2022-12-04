use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use arrow2::{
    array::{Array, Int64Array, ListArray},
    datatypes::Schema,
};
use polars::prelude::{DataFrame, Series};

use re_arrow_store::{datagen::*, ComponentNameRef, DataStore, TimeQuery, TypedTimeInt};
use re_log_types::{ObjPath as EntityPath, TimeType, Timeline};

// --- Scenarios ---

/// Covering a very common end-to-end use case:
/// - single entity path
/// - static set of instances
/// - multiple components uploaded at different rates
/// - multiple timelines with non-monotically increasing updates
/// - no weird stuff (duplicated components etc)
#[test]
fn single_entity_multi_timelines_multi_components_out_of_order_roundtrip() {
    let mut store = DataStore::default();

    let ent_path = EntityPath::from("this/that");

    let now = SystemTime::now();
    let now_minus_20ms = now - Duration::from_millis(20);
    let now_minus_10ms = now - Duration::from_millis(10);
    let now_plus_10ms = now + Duration::from_millis(20);
    let now_plus_20ms = now + Duration::from_millis(20);

    let frame40 = 40;
    let frame41 = 41;
    let frame42 = 42;
    let frame43 = 43;
    let frame44 = 44;

    let nb_instances = 3;

    let mut all_data = HashMap::new();
    {
        insert_data(
            &mut store,
            &mut all_data,
            &ent_path,
            [build_log_time(now_minus_20ms), build_frame_nr(frame43)],
            [build_rects(nb_instances)],
        );
        insert_data(
            &mut store,
            &mut all_data,
            &ent_path,
            [build_log_time(now_plus_20ms), build_frame_nr(frame41)],
            [build_instances(nb_instances), build_rects(nb_instances)],
        );
        insert_data(
            &mut store,
            &mut all_data,
            &ent_path,
            [build_log_time(now), build_frame_nr(frame42)],
            [build_instances(nb_instances), build_rects(nb_instances)],
        );
        insert_data(
            &mut store,
            &mut all_data,
            &ent_path,
            [build_log_time(now_minus_10ms), build_frame_nr(frame42)],
            [build_positions(nb_instances)],
        );
    }

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_log_time = Timeline::new("log_time", TimeType::Time);
    let components_all = &["instances", "rects", "positions"];

    // Scenario: query at frame #40, no data supposed to exist yet!
    // Expected: empty dataframe.
    assert_scenario(
        &mut store,
        &timeline_frame_nr,
        &TimeQuery::LatestAt(frame40),
        &ent_path,
        components_all,
        [],
    );

    // TODO(cmc): broken
    // // Scenario: query a non-existing entity path.
    // assert_scenario(
    //     &mut store,
    //     &timeline_frame_nr,
    //     &TimeQuery::LatestAt(frame44),
    //     &EntityPath::from("does/not/exist"),
    //     components_all,
    //     [],
    // );

    // Scenario: query a bunch of non-existing components.
    // Expected: empty dataframe.
    assert_scenario(
        &mut store,
        &timeline_frame_nr,
        &TimeQuery::LatestAt(frame44),
        &ent_path,
        &["they", "dont", "exist"],
        [],
    );

    // TODO(cmc): test log_times too!

    // Scenario: query all components at `last frame + 1`.
    // Expected: latest data for all components.
    assert_scenario(
        &mut store,
        &timeline_frame_nr,
        &TimeQuery::LatestAt(frame44),
        &ent_path,
        components_all,
        [
            (
                "instances",
                all_data
                    .remove(&("instances", TypedTimeInt::new_seq(frame42)))
                    .unwrap(),
            ),
            (
                "rects",
                all_data
                    .remove(&("rects", TypedTimeInt::new_seq(frame43)))
                    .unwrap(),
            ),
            (
                "positions",
                all_data
                    .remove(&("positions", TypedTimeInt::new_seq(frame42)))
                    .unwrap(),
            ),
        ],
    );
}

// --- Helpers ---

fn insert_data<const N: usize, const M: usize>(
    store: &mut DataStore,
    all_data: &mut HashMap<(ComponentNameRef<'_>, TypedTimeInt), Box<dyn Array>>,
    ent_path: &EntityPath,
    times: [(TypedTimeInt, Schema, Int64Array); N],
    components: [(ComponentNameRef<'static>, Schema, ListArray<i32>); M],
) {
    for (time, _, _) in &times {
        for (name, _, comp) in &components {
            assert!(all_data
                .insert((name, time.clone()), comp.clone().boxed())
                .is_none());
        }
    }

    let (schema, components) = build_message(ent_path, times, components);
    eprintln!("inserting into '{ent_path}':\nschema: {schema:#?}\ncomponents: {components:#?}");
    // eprintln!("---\ninserting into '{ent_path}': [log_time, frame_nr], [rects]");
    store.insert(&schema, &components).unwrap();
    eprintln!("{store}");
}

fn assert_scenario<const N: usize>(
    store: &mut DataStore,
    timeline: &Timeline,
    time_query: &TimeQuery,
    ent_path: &EntityPath,
    components: &[ComponentNameRef<'_>],
    expected: [(ComponentNameRef<'_>, Box<dyn Array>); N],
) {
    let df = store
        .query(timeline, time_query, ent_path, components)
        .unwrap();

    let series = expected
        .into_iter()
        .map(|(name, data)| Series::try_from((name, data)).unwrap())
        .collect::<Vec<_>>();
    let expected = DataFrame::new(series).unwrap();

    assert_eq!(expected, df);
}
