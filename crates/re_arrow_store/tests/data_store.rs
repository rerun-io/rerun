use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering::SeqCst},
};

use arrow2::{array::Array, datatypes::Schema};
use polars::prelude::{DataFrame, Series};

use re_arrow_store::{DataStore, DataStoreConfig, TimeInt, TimeQuery};
use re_log_types::{
    datagen::*, ComponentNameRef, Duration, ObjPath as EntityPath, Time, TimePoint, TimeType,
    Timeline,
};

// ---

const TEST_CONFIGS: &[DataStoreConfig] = &[
    DataStoreConfig::DEFAULT,
    DataStoreConfig {
        component_bucket_nb_rows: 0,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        component_bucket_nb_rows: 1,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        component_bucket_nb_rows: 2,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        component_bucket_nb_rows: 3,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        component_bucket_size_bytes: 0,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        component_bucket_size_bytes: 16,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        component_bucket_size_bytes: 32,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        component_bucket_size_bytes: 64,
        ..DataStoreConfig::DEFAULT
    },
];

// --- Scenarios ---

#[test]
fn empty_query_edge_cases() {
    init_logs();

    for config in TEST_CONFIGS {
        let mut store = DataStore::new(config.clone());
        empty_query_edge_cases_impl(&mut store);
    }
}
fn empty_query_edge_cases_impl(store: &mut DataStore) {
    let ent_path = EntityPath::from("this/that");
    let now = Time::now();
    let now_nanos = now.nanos_since_epoch();
    let now_minus_10ms = now - Duration::from_millis(10);
    let now_minus_10ms_nanos = now_minus_10ms.nanos_since_epoch();
    let frame39 = 39;
    let frame40 = 40;
    let nb_instances = 3;

    let mut tracker = DataTracker::default();
    {
        tracker.insert_data(
            store,
            &ent_path,
            [build_log_time(now), build_frame_nr(frame40)],
            [build_instances(nb_instances)],
        );
    }

    let timeline_wrong_name = Timeline::new("lag_time", TimeType::Time);
    let timeline_wrong_kind = Timeline::new("log_time", TimeType::Sequence);
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_log_time = Timeline::new("log_time", TimeType::Time);
    let components_all = &["instances"];

    // Scenario: query at `last_frame`.
    // Expected: dataframe with our instances in it.
    tracker.assert_scenario(
        store,
        &timeline_frame_nr,
        &TimeQuery::LatestAt(frame40),
        &ent_path,
        components_all,
        vec![("instances", frame40.into())],
    );

    // Scenario: query at `last_log_time`.
    // Expected: dataframe with our instances in it.
    tracker.assert_scenario(
        store,
        &timeline_log_time,
        &TimeQuery::LatestAt(now_nanos),
        &ent_path,
        components_all,
        vec![("instances", now_nanos.into())],
    );

    // Scenario: query an empty store at `first_frame - 1`.
    // Expected: empty dataframe.
    tracker.assert_scenario(
        store,
        &timeline_frame_nr,
        &TimeQuery::LatestAt(frame39),
        &ent_path,
        components_all,
        vec![],
    );

    // Scenario: query an empty store at `first_log_time - 10ms`.
    // Expected: empty dataframe.
    tracker.assert_scenario(
        store,
        &timeline_log_time,
        &TimeQuery::LatestAt(now_minus_10ms_nanos),
        &ent_path,
        components_all,
        vec![],
    );

    // Scenario: query a non-existing entity path.
    // Expected: empty dataframe.
    tracker.assert_scenario(
        store,
        &timeline_frame_nr,
        &TimeQuery::LatestAt(frame40),
        &EntityPath::from("does/not/exist"),
        components_all,
        vec![],
    );

    // Scenario: query a bunch of non-existing components.
    // Expected: empty dataframe.
    tracker.assert_scenario(
        store,
        &timeline_frame_nr,
        &TimeQuery::LatestAt(frame40),
        &ent_path,
        &["they", "dont", "exist"],
        vec![],
    );

    // Scenario: query with an empty list of components.
    // Expected: empty dataframe.
    tracker.assert_scenario(
        store,
        &timeline_frame_nr,
        &TimeQuery::LatestAt(frame40),
        &ent_path,
        &[],
        vec![],
    );

    // Scenario: query with wrong timeline name.
    // Expected: empty dataframe.
    tracker.assert_scenario(
        store,
        &timeline_wrong_name,
        &TimeQuery::LatestAt(frame40),
        &ent_path,
        components_all,
        vec![],
    );

    // Scenario: query with wrong timeline kind.
    // Expected: empty dataframe.
    tracker.assert_scenario(
        store,
        &timeline_wrong_kind,
        &TimeQuery::LatestAt(frame40),
        &ent_path,
        components_all,
        vec![],
    );
}

/// Covering a very common end-to-end use case:
/// - single entity path
/// - static set of instances
/// - multiple components uploaded at different rates
/// - multiple timelines with non-monotically increasing updates
/// - no weird stuff (duplicated components etc)
#[test]
fn end_to_end_roundtrip_standard() {
    init_logs();

    for config in TEST_CONFIGS {
        let mut store = DataStore::new(config.clone());
        end_to_end_roundtrip_standard_impl(&mut store);
    }
}
fn end_to_end_roundtrip_standard_impl(store: &mut DataStore) {
    let ent_path = EntityPath::from("this/that");

    let now = Time::now();
    let now_minus_10ms = now - Duration::from_millis(10);
    let now_minus_10ms_nanos = now_minus_10ms.nanos_since_epoch();
    let now_plus_10ms = now + Duration::from_millis(10);
    let now_plus_10ms_nanos = now_plus_10ms.nanos_since_epoch();
    let now_plus_20ms = now + Duration::from_millis(20);

    let frame40 = 40;
    let frame41 = 41;
    let frame42 = 42;
    let frame43 = 43;
    let frame44 = 44;

    let nb_instances = 3;

    let mut tracker = DataTracker::default();
    {
        tracker.insert_data(
            store,
            &ent_path,
            [build_log_time(now_minus_10ms), build_frame_nr(frame43)],
            [build_rects(nb_instances)],
        );
        tracker.insert_data(
            store,
            &ent_path,
            [build_log_time(now), build_frame_nr(frame42)],
            [build_rects(nb_instances)],
        );
        tracker.insert_data(
            store,
            &ent_path,
            [build_log_time(now_plus_10ms), build_frame_nr(frame41)],
            [build_instances(nb_instances), build_rects(nb_instances)],
        );
        tracker.insert_data(
            store,
            &ent_path,
            [build_log_time(now), build_frame_nr(frame42)],
            [build_instances(nb_instances)],
        );
        tracker.insert_data(
            store,
            &ent_path,
            [build_log_time(now_minus_10ms), build_frame_nr(frame42)],
            [build_positions(nb_instances)],
        );
    }

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_log_time = Timeline::new("log_time", TimeType::Time);
    let components_all = &["instances", "rects", "positions"];

    // --- Testing at all frames ---

    let scenarios = [
        // Scenario: query all components at frame #40 (i.e. before first frame).
        // Expected: empy dataframe.
        (frame40, vec![]),
        // Scenario: query all components at frame #41 (i.e. first frame with data)
        // Expected: data at that point in time.
        (
            frame41,
            vec![("instances", frame41.into()), ("rects", frame41.into())],
        ),
        // Scenario: query all components at frame #42 (i.e. second frame with data)
        // Expected: data at that point in time.
        (
            frame42,
            vec![
                ("instances", frame42.into()),
                ("rects", frame42.into()),
                ("positions", frame42.into()),
            ],
        ),
        // Scenario: query all components at frame #43 (i.e. last frame with data)
        // Expected: latest data for all components.
        (
            frame43,
            vec![
                ("instances", frame42.into()),
                ("rects", frame43.into()),
                ("positions", frame42.into()),
            ],
        ),
        // Scenario: query all components at frame #44 (i.e. after last frame)
        // Expected: latest data for all components.
        (
            frame44,
            vec![
                ("instances", frame42.into()),
                ("rects", frame43.into()),
                ("positions", frame42.into()),
            ],
        ),
    ];

    for (frame_nr, expected) in scenarios {
        tracker.assert_scenario(
            store,
            &timeline_frame_nr,
            &TimeQuery::LatestAt(frame_nr),
            &ent_path,
            components_all,
            expected,
        );
    }

    // --- Testing at all times ---

    // TODO(cmc): test log_times -10, +0, +10, +20

    let scenarios = [
        // Scenario: query all components at +20ms (i.e. after last update).
        // Expected: latest data for all components.
        (
            now_plus_20ms,
            vec![
                ("instances", now_plus_10ms_nanos.into()),
                ("rects", now_plus_10ms_nanos.into()),
                ("positions", now_minus_10ms_nanos.into()),
            ],
        ),
    ];

    for (log_time, expected) in scenarios {
        tracker.assert_scenario(
            store,
            &timeline_log_time,
            &TimeQuery::LatestAt(log_time.nanos_since_epoch()),
            &ent_path,
            components_all,
            expected,
        );
    }
}

// --- Helpers ---

type DataEntry = (ComponentNameRef<'static>, TimeInt);

#[derive(Default)]
struct DataTracker {
    all_data: HashMap<(ComponentNameRef<'static>, TimeInt), Box<dyn Array>>,
}

impl DataTracker {
    fn insert_data<const N: usize, const M: usize>(
        &mut self,
        store: &mut DataStore,
        ent_path: &EntityPath,
        times: [(Timeline, TimeInt); N],
        components: [(ComponentNameRef<'static>, Schema, Box<dyn Array>); M],
    ) {
        let timepoint = TimePoint::from(times);

        for time in timepoint.times() {
            for (name, _, comp) in &components {
                assert!(self.all_data.insert((name, *time), comp.clone()).is_none());
            }
        }

        let (schema, components) = build_message(ent_path, &timepoint, components);
        // eprintln!("inserting into '{ent_path}':\nschema: {schema:#?}\ncomponents: {components:#?}");
        // eprintln!("---\ninserting into '{ent_path}': [log_time, frame_nr], [rects]");
        store.insert(&schema, &components).unwrap();
        // eprintln!("{store}");
    }

    fn assert_scenario(
        &self,
        store: &mut DataStore,
        timeline: &Timeline,
        time_query: &TimeQuery,
        ent_path: &EntityPath,
        components: &[ComponentNameRef<'_>],
        expected: Vec<DataEntry>,
    ) {
        let df = store
            .query(timeline, time_query, ent_path, components)
            .unwrap();

        let series = expected
            .into_iter()
            .map(|(name, time)| (name, self.all_data[&(name, time)].clone()))
            .map(|(name, data)| Series::try_from((name, data)).unwrap())
            .collect::<Vec<_>>();
        let expected = DataFrame::new(series).unwrap();
        let expected = expected.explode(expected.get_column_names()).unwrap();

        store.sort_indices();
        eprintln!("{store}");
        assert_eq!(expected, df);
    }
}

fn init_logs() {
    static INIT: AtomicBool = AtomicBool::new(false);

    if INIT.compare_exchange(false, true, SeqCst, SeqCst).is_ok() {
        re_log::set_default_rust_log_env();
        tracing_subscriber::fmt::init(); // log to stdout
    }
}
