use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering::SeqCst},
};

use arrow2::{array::Array, datatypes::Schema};
use polars::prelude::{DataFrame, Series};

use re_arrow_store::{DataStore, DataStoreConfig, TimeInt, TimeQuery};
use re_log_types::{
    datagen::{
        build_frame_nr, build_instances, build_log_time, build_message, build_positions,
        build_rects,
    },
    ComponentNameRef, Duration, ObjPath as EntityPath, Time, TimePoint, TimeType, Timeline,
};

// --- Configs ---

const COMPONENT_CONFIGS: &[DataStoreConfig] = &[
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

const INDEX_CONFIGS: &[DataStoreConfig] = &[
    DataStoreConfig::DEFAULT,
    DataStoreConfig {
        index_bucket_nb_rows: 0,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        index_bucket_nb_rows: 1,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        index_bucket_nb_rows: 2,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        index_bucket_nb_rows: 3,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        index_bucket_size_bytes: 0,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        index_bucket_size_bytes: 16,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        index_bucket_size_bytes: 32,
        ..DataStoreConfig::DEFAULT
    },
    DataStoreConfig {
        index_bucket_size_bytes: 64,
        ..DataStoreConfig::DEFAULT
    },
];

fn all_configs() -> impl Iterator<Item = DataStoreConfig> {
    COMPONENT_CONFIGS.iter().flat_map(|comp| {
        INDEX_CONFIGS.iter().map(|idx| DataStoreConfig {
            component_bucket_size_bytes: comp.component_bucket_size_bytes,
            component_bucket_nb_rows: comp.component_bucket_nb_rows,
            index_bucket_size_bytes: idx.index_bucket_size_bytes,
            index_bucket_nb_rows: idx.index_bucket_nb_rows,
        })
    })
}

// --- Scenarios ---

#[test]
fn empty_query_edge_cases() {
    init_logs();

    for config in all_configs() {
        let mut store = DataStore::new(config.clone());
        empty_query_edge_cases_impl(&mut store);
    }
}
fn empty_query_edge_cases_impl(store: &mut DataStore) {
    let ent_path = EntityPath::from("this/that");
    let now = Time::now();
    let now_nanos = now.nanos_since_epoch();
    let now_minus_1s = now - Duration::from_secs(1.0);
    let now_minus_1s_nanos = now_minus_1s.nanos_since_epoch();
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

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
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

    // Scenario: query an empty store at `first_log_time - 1s`.
    // Expected: empty dataframe.
    tracker.assert_scenario(
        store,
        &timeline_log_time,
        &TimeQuery::LatestAt(now_minus_1s_nanos),
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

    for config in all_configs() {
        let mut store = DataStore::new(config.clone());
        end_to_end_roundtrip_standard_impl(&mut store);
    }
}
fn end_to_end_roundtrip_standard_impl(store: &mut DataStore) {
    let ent_path = EntityPath::from("this/that");

    let now = Time::now();
    let now_nanos = now.nanos_since_epoch();
    let now_minus_2s = now - Duration::from_secs(2.0);
    let now_minus_1s = now - Duration::from_secs(1.0);
    let now_minus_1s_nanos = now_minus_1s.nanos_since_epoch();
    let now_plus_1s = now + Duration::from_secs(1.0);
    let now_plus_1s_nanos = now_plus_1s.nanos_since_epoch();
    let now_plus_2s = now + Duration::from_secs(2.0);

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
            [build_frame_nr(frame41)],
            [build_instances(nb_instances)],
        );
        tracker.insert_data(
            store,
            &ent_path,
            [build_frame_nr(frame41)],
            [build_positions(nb_instances)],
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
            [build_log_time(now_plus_1s)],
            [build_instances(nb_instances), build_rects(nb_instances)],
        );
        tracker.insert_data(
            store,
            &ent_path,
            [build_frame_nr(frame41)],
            [build_rects(nb_instances)],
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
            [build_log_time(now_minus_1s), build_frame_nr(frame42)],
            [build_positions(nb_instances)],
        );
        tracker.insert_data(
            store,
            &ent_path,
            [build_log_time(now_minus_1s), build_frame_nr(frame43)],
            [build_rects(nb_instances)],
        );
        tracker.insert_data(
            store,
            &ent_path,
            [build_frame_nr(frame44)],
            [build_positions(nb_instances)],
        );
    }

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
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
            vec![
                ("instances", frame41.into()),
                ("rects", frame41.into()),
                ("positions", frame41.into()),
            ],
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
                ("positions", frame44.into()),
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

    let scenarios = [
        // Scenario: query all components at -2s (i.e. before first update).
        // Expected: empty dataframe.
        (now_minus_2s, vec![]),
        // Scenario: query all components at -1s (i.e. first update).
        // Expected: data at that point in time.
        (
            now_minus_1s,
            vec![
                ("rects", now_minus_1s_nanos.into()),
                ("positions", now_minus_1s_nanos.into()),
            ],
        ),
        // Scenario: query all components at 0s (i.e. second update).
        // Expected: data at that point in time.
        (
            now,
            vec![
                ("instances", now_nanos.into()),
                ("rects", now_nanos.into()),
                ("positions", now_minus_1s_nanos.into()),
            ],
        ),
        // Scenario: query all components at +1s (i.e. last update).
        // Expected: latest data for all components.
        (
            now_plus_1s,
            vec![
                ("instances", now_plus_1s_nanos.into()),
                ("rects", now_plus_1s_nanos.into()),
                ("positions", now_minus_1s_nanos.into()),
            ],
        ),
        // Scenario: query all components at +2s (i.e. after last update).
        // Expected: latest data for all components.
        (
            now_plus_2s,
            vec![
                ("instances", now_plus_1s_nanos.into()),
                ("rects", now_plus_1s_nanos.into()),
                ("positions", now_minus_1s_nanos.into()),
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

#[derive(Default)]
struct DataTracker {
    all_data: HashMap<(ComponentNameRef<'static>, TimeInt), Vec<Box<dyn Array>>>,
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
                let comps = self.all_data.entry((name, *time)).or_default();
                comps.push(comp.clone());
            }
        }

        let (schema, components) = build_message(ent_path, &timepoint, components);
        // eprintln!("inserting into '{ent_path}':\nschema: {schema:#?}\ncomponents: {components:#?}");
        // eprintln!("---\ninserting into '{ent_path}': [log_time, frame_nr], [rects]");
        store.insert(&schema, &components).unwrap();
        // eprintln!("{store}");
    }

    /// Asserts a simple scenario, where every component is fetched from its own point-of-view.
    fn assert_scenario(
        &self,
        store: &mut DataStore,
        timeline: &Timeline,
        time_query: &TimeQuery,
        ent_path: &EntityPath,
        components: &[ComponentNameRef<'_>],
        expected: Vec<(ComponentNameRef<'static>, TimeInt)>,
    ) {
        let df = {
            let series = components
                .iter()
                .filter_map(|&component| {
                    Self::query_component_pov(
                        store, timeline, time_query, ent_path, component, component,
                    )
                })
                .collect::<Vec<_>>();

            let df = DataFrame::new(series).unwrap();
            df.explode(df.get_column_names()).unwrap()
        };

        let series = expected
            .into_iter()
            .map(|(name, time)| (name, self.all_data[&(name, time)].clone()))
            .map(|(name, data)| Series::try_from((name, data)).unwrap())
            .collect::<Vec<_>>();
        let expected = DataFrame::new(series).unwrap();
        let expected = expected.explode(expected.get_column_names()).unwrap();

        store.sort_indices();
        assert_eq!(expected, df, "\n{store}");
    }

    /// Asserts a complex scenario, where every component is fetched as it is seen from the
    /// point-of-view of another component.
    fn assert_scenario_pov(
        &self,
        store: &mut DataStore,
        timeline: &Timeline,
        time_query: &TimeQuery,
        ent_path: &EntityPath,
        primary: ComponentNameRef<'_>,
        components: &[(ComponentNameRef<'_>, ComponentNameRef<'_>)], // (primary, component)
        expected: Vec<(ComponentNameRef<'static>, TimeInt, usize)>,
    ) {
        let df = {
            let series = components
                .iter()
                .filter_map(|(primary, component)| {
                    Self::query_component_pov(
                        store, timeline, time_query, ent_path, primary, component,
                    )
                })
                .collect::<Vec<_>>();

            let df = DataFrame::new(series).unwrap();
            df.explode(df.get_column_names()).unwrap()
        };

        let series = expected
            .into_iter()
            .map(|(name, time, idx)| (name, self.all_data[&(name, time)][idx].clone()))
            .map(|(name, data)| Series::try_from((name, data)).unwrap())
            .collect::<Vec<_>>();
        let expected = DataFrame::new(series).unwrap();
        let expected = expected.explode(expected.get_column_names()).unwrap();

        store.sort_indices();
        assert_eq!(expected, df, "\n{store}");
    }

    fn query_component_pov(
        store: &DataStore,
        timeline: &Timeline,
        time_query: &TimeQuery,
        ent_path: &EntityPath,
        primary: ComponentNameRef<'_>,
        component: ComponentNameRef<'_>,
    ) -> Option<Series> {
        let mut row_indices = [None];
        store.query(
            timeline,
            time_query,
            ent_path,
            primary,
            &[component],
            &mut row_indices,
        );

        let mut results = [None];
        store.get(&[component], &row_indices, &mut results);

        std::mem::take(&mut results[0]).map(|row| Series::try_from((component, row)).unwrap())
    }
}

fn init_logs() {
    static INIT: AtomicBool = AtomicBool::new(false);

    if INIT.compare_exchange(false, true, SeqCst, SeqCst).is_ok() {
        re_log::set_default_rust_log_env();
        tracing_subscriber::fmt::init(); // log to stdout
    }
}

// --- Internals ---

// TODO(cmc): One should _never_ run assertions on the internal state of the datastore, this
// is a recipe for disaster.
//
// The contract that needs to be asserted here, from the point of view of the actual user,
// is performance: getting the datastore into a pathological topology should show up in
// integration query benchmarks.
//
// In the current state of things, though, it is much easier to test for it that way... so we
// make an exception, for now...
#[test]
fn pathological_bucket_topology() {
    init_logs();

    let ent_path = EntityPath::from("this/that");
    let nb_instances = 1;

    let mut store_forward = DataStore::new(DataStoreConfig {
        index_bucket_nb_rows: 10,
        ..Default::default()
    });
    let mut store_backward = DataStore::new(DataStoreConfig {
        index_bucket_nb_rows: 10,
        ..Default::default()
    });

    {
        let timepoint = TimePoint::from([build_frame_nr(1000)]);
        for _ in 0..10 {
            let (schema, components) =
                build_message(&ent_path, &timepoint, [build_instances(nb_instances)]);
            store_forward.insert(&schema, &components).unwrap();
            store_backward.insert(&schema, &components).unwrap();
        }
    }

    let msgs = (970..=979)
        .map(|frame_nr| {
            let timepoint = TimePoint::from([build_frame_nr(frame_nr)]);
            build_message(&ent_path, &timepoint, [build_instances(nb_instances)])
        })
        .collect::<Vec<_>>();
    for (schema, components) in &msgs {
        store_forward.insert(schema, components).unwrap();
    }
    for (schema, components) in msgs.iter().rev() {
        store_backward.insert(schema, components).unwrap();
    }

    let msgs = (990..=999)
        .map(|frame_nr| {
            let timepoint = TimePoint::from([build_frame_nr(frame_nr)]);
            build_message(&ent_path, &timepoint, [build_instances(nb_instances)])
        })
        .collect::<Vec<_>>();
    for (schema, components) in &msgs {
        store_forward.insert(schema, components).unwrap();
    }
    for (schema, components) in msgs.iter().rev() {
        store_backward.insert(schema, components).unwrap();
    }

    let msgs = (980..=989)
        .map(|frame_nr| {
            let timepoint = TimePoint::from([build_frame_nr(frame_nr)]);
            build_message(&ent_path, &timepoint, [build_instances(nb_instances)])
        })
        .collect::<Vec<_>>();
    for (schema, components) in &msgs {
        store_forward.insert(schema, components).unwrap();
    }
    for (schema, components) in msgs.iter().rev() {
        store_backward.insert(schema, components).unwrap();
    }

    {
        let timepoint = TimePoint::from([build_frame_nr(1000)]);
        for _ in 0..7 {
            let (schema, components) =
                build_message(&ent_path, &timepoint, [build_instances(nb_instances)]);
            store_forward.insert(&schema, &components).unwrap();
            store_backward.insert(&schema, &components).unwrap();
        }
    }

    let msgs = (1000..=1009)
        .map(|frame_nr| {
            let timepoint = TimePoint::from([build_frame_nr(frame_nr)]);
            build_message(&ent_path, &timepoint, [build_instances(nb_instances)])
        })
        .collect::<Vec<_>>();
    for (schema, components) in &msgs {
        store_forward.insert(schema, components).unwrap();
    }
    for (schema, components) in msgs.iter().rev() {
        store_backward.insert(schema, components).unwrap();
    }

    {
        let timepoint = TimePoint::from([build_frame_nr(975)]);
        for _ in 0..10 {
            let (schema, components) =
                build_message(&ent_path, &timepoint, [build_instances(nb_instances)]);
            store_forward.insert(&schema, &components).unwrap();
            store_backward.insert(&schema, &components).unwrap();
        }
    }

    {
        let nb_buckets = store_forward
            .iter_indices()
            .flat_map(|(_, table)| table.iter_buckets())
            .count();
        assert_eq!(7usize, nb_buckets, "pathological topology (forward): {}", {
            store_forward.sort_indices();
            store_forward
        });
    }
    {
        let nb_buckets = store_backward
            .iter_indices()
            .flat_map(|(_, table)| table.iter_buckets())
            .count();
        assert_eq!(
            8usize,
            nb_buckets,
            "pathological topology (backward): {}",
            {
                store_backward.sort_indices();
                store_backward
            }
        );
    }
}
