use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering::SeqCst},
};

use arrow2::array::Array;

use polars_core::{prelude::DataFrame, series::Series};
use re_arrow_store::{DataStore, DataStoreConfig, TimeInt, TimeQuery, TimelineQuery};
use re_log_types::{
    datagen::{
        build_frame_nr, build_instances, build_log_time, build_some_point2d, build_some_rects,
    },
    field_types::{Instance, Point2D, Rect2D},
    msg_bundle::{Component, ComponentBundle, MsgBundle},
    ComponentName, ComponentNameRef, Duration, MsgId, ObjPath as EntityPath, Time, TimePoint,
    TimeType, Timeline,
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

macro_rules! test_bundle {
    ($entity:ident @ $frames:tt => [$c0:expr $(,)*]) => {
        re_log_types::msg_bundle::try_build_msg_bundle1(MsgId::ZERO, $entity.clone(), $frames, $c0)
            .unwrap()
    };
    ($entity:ident @ $frames:tt => [$c0:expr, $c1:expr $(,)*]) => {
        re_log_types::msg_bundle::try_build_msg_bundle2(
            MsgId::ZERO,
            $entity.clone(),
            $frames,
            ($c0, $c1),
        )
        .unwrap()
    };
}

#[test]
fn empty_query_edge_cases() {
    init_logs();

    for config in all_configs() {
        let mut store = DataStore::new(Instance::NAME.to_owned(), config.clone());
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
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_log_time(now), build_frame_nr(frame40)] => [
                build_instances(nb_instances),
            ]),
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
    let components_all = &[Instance::NAME];

    tracker.assert_scenario(
        "query at `last_frame`",
        "dataframe with our instances in it",
        store,
        &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame40)),
        &ent_path,
        components_all,
        vec![(Instance::NAME, frame40.into())],
    );

    tracker.assert_scenario(
        "query at `last_log_time`",
        "dataframe with our instances in it",
        store,
        &TimelineQuery::new(timeline_log_time, TimeQuery::LatestAt(now_nanos)),
        &ent_path,
        components_all,
        vec![(Instance::NAME, now_nanos.into())],
    );

    tracker.assert_scenario(
        "query an empty store at `first_frame - 1`",
        "empty dataframe",
        store,
        &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame39)),
        &ent_path,
        components_all,
        vec![],
    );

    tracker.assert_scenario(
        "query an empty store at `first_log_time - 1s`",
        "empty dataframe",
        store,
        &TimelineQuery::new(timeline_log_time, TimeQuery::LatestAt(now_minus_1s_nanos)),
        &ent_path,
        components_all,
        vec![],
    );

    tracker.assert_scenario(
        "query a non-existing entity path",
        "empty dataframe",
        store,
        &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame40)),
        &EntityPath::from("does/not/exist"),
        components_all,
        vec![],
    );

    tracker.assert_scenario(
        "query a bunch of non-existing components",
        "empty dataframe",
        store,
        &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame40)),
        &ent_path,
        &["they", "dont", "exist"],
        vec![],
    );

    tracker.assert_scenario(
        "query with an empty list of components",
        "empty dataframe",
        store,
        &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame40)),
        &ent_path,
        &[],
        vec![],
    );

    tracker.assert_scenario(
        "query with wrong timeline name",
        "empty dataframe",
        store,
        &TimelineQuery::new(timeline_wrong_name, TimeQuery::LatestAt(frame40)),
        &ent_path,
        components_all,
        vec![],
    );

    tracker.assert_scenario(
        "query with wrong timeline kind",
        "empty dataframe",
        store,
        &TimelineQuery::new(timeline_wrong_kind, TimeQuery::LatestAt(frame40)),
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
        let mut store = DataStore::new(Instance::NAME.to_owned(), config.clone());
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
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_frame_nr(frame41)] => [
                build_instances(nb_instances),
            ]),
        );
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_frame_nr(frame41)] => [
                build_some_point2d(nb_instances),
            ]),
        );
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_log_time(now), build_frame_nr(frame42)] => [
                build_some_rects(nb_instances),
            ]),
        );
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_log_time(now_plus_1s)] => [
                build_instances(nb_instances),
                build_some_rects(nb_instances),
            ]),
        );
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_frame_nr(frame41)] => [
                build_some_rects(nb_instances),
            ]),
        );
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_log_time(now), build_frame_nr(frame42)] => [
                build_instances(nb_instances),
            ]),
        );
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_log_time(now_minus_1s), build_frame_nr(frame42)] => [
                build_some_point2d(nb_instances),
            ]),
        );
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_log_time(now_minus_1s), build_frame_nr(frame43)] => [
                build_some_rects(nb_instances),
            ]),
        );
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_frame_nr(frame44)] => [
                build_some_point2d(nb_instances),
            ]),
        );
    }

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
    }

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_log_time = Timeline::new("log_time", TimeType::Time);
    let components_all = &[Instance::NAME, Rect2D::NAME, Point2D::NAME];

    // --- Testing at all frames ---

    let scenarios = [
        (
            "query all components at frame #40 (i.e. before first frame)",
            "empy dataframe",
            frame40,
            vec![],
        ),
        (
            "query all components at frame #41 (i.e. first frame with data)",
            "data at that point in time",
            frame41,
            vec![
                (Instance::NAME, frame41.into()),
                (Rect2D::NAME, frame41.into()),
                (Point2D::NAME, frame41.into()),
            ],
        ),
        (
            "query all components at frame #42 (i.e. second frame with data)",
            "data at that point in time",
            frame42,
            vec![
                (Instance::NAME, frame42.into()),
                (Rect2D::NAME, frame42.into()),
                (Point2D::NAME, frame42.into()),
            ],
        ),
        (
            "query all components at frame #43 (i.e. last frame with data)",
            "latest data for all components",
            frame43,
            vec![
                (Instance::NAME, frame42.into()),
                (Rect2D::NAME, frame43.into()),
                (Point2D::NAME, frame42.into()),
            ],
        ),
        (
            "query all components at frame #44 (i.e. after last frame)",
            "latest data for all components",
            frame44,
            vec![
                (Instance::NAME, frame42.into()),
                (Rect2D::NAME, frame43.into()),
                (Point2D::NAME, frame44.into()),
            ],
        ),
    ];

    for (scenario, expectation, frame_nr, expected) in scenarios {
        tracker.assert_scenario(
            scenario,
            expectation,
            store,
            &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame_nr)),
            &ent_path,
            components_all,
            expected,
        );
    }

    // --- Testing at all times ---

    let scenarios = [
        (
            "query all components at -2s (i.e. before first update)",
            "empty dataframe",
            now_minus_2s,
            vec![],
        ),
        (
            "query all components at -1s (i.e. first update)",
            "data at that point in time",
            now_minus_1s,
            vec![
                (Rect2D::NAME, now_minus_1s_nanos.into()),
                (Point2D::NAME, now_minus_1s_nanos.into()),
            ],
        ),
        (
            "query all components at 0s (i.e. second update)",
            "data at that point in time",
            now,
            vec![
                (Instance::NAME, now_nanos.into()),
                (Rect2D::NAME, now_nanos.into()),
                (Point2D::NAME, now_minus_1s_nanos.into()),
            ],
        ),
        (
            "query all components at +1s (i.e. last update)",
            "latest data for all components",
            now_plus_1s,
            vec![
                (Instance::NAME, now_plus_1s_nanos.into()),
                (Rect2D::NAME, now_plus_1s_nanos.into()),
                (Point2D::NAME, now_minus_1s_nanos.into()),
            ],
        ),
        (
            "query all components at +2s (i.e. after last update)",
            "latest data for all components",
            now_plus_2s,
            vec![
                (Instance::NAME, now_plus_1s_nanos.into()),
                (Rect2D::NAME, now_plus_1s_nanos.into()),
                (Point2D::NAME, now_minus_1s_nanos.into()),
            ],
        ),
    ];

    for (scenario, expectation, log_time, expected) in scenarios {
        tracker.assert_scenario(
            scenario,
            expectation,
            store,
            &TimelineQuery::new(
                timeline_log_time,
                TimeQuery::LatestAt(log_time.nanos_since_epoch()),
            ),
            &ent_path,
            components_all,
            expected,
        );
    }
}

#[test]
fn query_model_specifics() {
    init_logs();

    for config in all_configs() {
        let mut store = DataStore::new(Instance::NAME.to_owned(), config.clone());
        query_model_specifics_impl(&mut store);
    }
}
fn query_model_specifics_impl(store: &mut DataStore) {
    let ent_path = EntityPath::from("this/that");

    let frame40 = 40;
    let frame41 = 41;
    let frame42 = 42;

    let nb_rects = 3;
    let nb_positions_before = 10;
    let nb_positions_after = 2;

    let mut tracker = DataTracker::default();
    {
        // PoV queries
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_frame_nr(frame41)] => [
                build_instances(nb_rects),
                build_some_rects(nb_rects),
            ]),
        );
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_frame_nr(frame41)] => [
                build_instances(nb_positions_before),
                build_some_point2d(nb_positions_before),
            ]),
        );

        // "Sparse but no diffs"
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_frame_nr(frame42)] => [
                build_instances(nb_positions_after),
                build_some_point2d(nb_positions_after),
            ]),
        );
        tracker.insert_bundle(
            store,
            &test_bundle!(ent_path @ [build_frame_nr(frame42)] => [
                build_some_rects(nb_rects),
            ]),
        );
    }

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
    }

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let components_all = &[Instance::NAME, Rect2D::NAME, Point2D::NAME];

    let scenarios = [
        (
            "query all components at frame #40, from `rects` PoV",
            "empty dataframe",
            frame40,
            Rect2D::NAME,
            vec![],
        ),
        (
            "query all components at frame #40, from `positions` PoV",
            "empty dataframe",
            frame40,
            Point2D::NAME,
            vec![],
        ),
        (
            "query all components at frame #40, from `instances` PoV",
            "empty dataframe",
            frame40,
            Instance::NAME,
            vec![],
        ),
        (
            "query all components at frame #41, from `rects` PoV",
            "the set of `rects` and the _first_ set of `instances` at that time",
            frame41,
            Rect2D::NAME,
            vec![
                (Instance::NAME, frame41.into(), 0),
                (Rect2D::NAME, frame41.into(), 0),
            ],
        ),
        (
            "query all components at frame #41, from `positions` PoV",
            "the set of `positions` and the _second_ set of `instances` at that time",
            frame41,
            Point2D::NAME,
            vec![
                (Instance::NAME, frame41.into(), 1),
                (Point2D::NAME, frame41.into(), 0),
            ],
        ),
        (
            "query all components at frame #41, from `instances` PoV",
            "the _second_ set of `instances` and the set of `positions` at that time",
            frame41,
            Instance::NAME,
            vec![
                (Instance::NAME, frame41.into(), 1),
                (Point2D::NAME, frame41.into(), 0),
            ],
        ),
        (
            "query all components at frame #42, from `positions` PoV",
            "the set of `positions` and the set of `instances` at that time",
            frame42,
            Point2D::NAME,
            vec![
                (Instance::NAME, frame42.into(), 0),
                (Point2D::NAME, frame42.into(), 0),
            ],
        ),
        (
            "query all components at frame #42, from `rects` PoV",
            "the set of `rects` at that time",
            frame42,
            Rect2D::NAME,
            vec![(Rect2D::NAME, frame42.into(), 0)],
        ),
        (
            "query all components at frame #42, from `instances` PoV",
            "the set of `positions` and the set of `instances` at that time",
            frame42,
            Instance::NAME,
            vec![
                (Instance::NAME, frame42.into(), 0),
                (Point2D::NAME, frame42.into(), 0),
            ],
        ),
    ];

    for (scenario, expectation, frame_nr, primary, expected) in scenarios {
        tracker.assert_scenario_pov(
            scenario,
            expectation,
            store,
            &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame_nr)),
            &ent_path,
            primary,
            components_all,
            expected,
        );
    }
}

// --- Helpers ---

#[derive(Default)]
struct DataTracker {
    all_data: HashMap<(ComponentName, TimeInt), Vec<Box<dyn Array>>>,
}

impl DataTracker {
    fn insert_bundle(&mut self, store: &mut DataStore, msg_bundle: &MsgBundle) {
        for time in msg_bundle.time_point.times() {
            for bundle in &msg_bundle.components {
                let ComponentBundle { name, value } = bundle;
                let comps = self.all_data.entry((name.clone(), *time)).or_default();
                comps.push(value.clone());
            }
        }
        store.insert(msg_bundle).unwrap();
    }

    /// Asserts a simple scenario, where every component is fetched from its own point-of-view.
    #[allow(clippy::too_many_arguments)]
    fn assert_scenario<const N: usize>(
        &self,
        scenario: &str,
        expectation: &str,
        store: &mut DataStore,
        timeline_query: &TimelineQuery,
        ent_path: &EntityPath,
        components: &[ComponentNameRef<'_>; N],
        expected: Vec<(ComponentNameRef<'static>, TimeInt)>,
    ) {
        self.assert_scenario_pov_impl(
            scenario,
            expectation,
            store,
            timeline_query,
            ent_path,
            None,
            components,
            expected
                .into_iter()
                .map(|(name, time)| (name, time, 0))
                .collect(),
        );
    }

    /// Asserts a pov scenario, where every component is fetched as it is seen from the
    /// point-of-view of another component.
    #[allow(clippy::too_many_arguments)]
    fn assert_scenario_pov<const N: usize>(
        &self,
        scenario: &str,
        expectation: &str,
        store: &mut DataStore,
        timeline_query: &TimelineQuery,
        ent_path: &EntityPath,
        primary: ComponentNameRef<'_>,
        components: &[ComponentNameRef<'_>; N],
        expected: Vec<(ComponentNameRef<'static>, TimeInt, usize)>,
    ) {
        self.assert_scenario_pov_impl(
            scenario,
            expectation,
            store,
            timeline_query,
            ent_path,
            primary.into(),
            components,
            expected,
        );
    }

    /// Asserts a complex scenario, where every component is either fetched as it is seen from the
    /// point-of-view of another component, or its own point-of-view if primary is None.
    #[allow(clippy::too_many_arguments)]
    fn assert_scenario_pov_impl<const N: usize>(
        &self,
        scenario: &str,
        expectation: &str,
        store: &mut DataStore,
        timeline_query: &TimelineQuery,
        ent_path: &EntityPath,
        primary: Option<ComponentNameRef<'_>>,
        components: &[ComponentNameRef<'_>; N],
        expected: Vec<(ComponentNameRef<'static>, TimeInt, usize)>,
    ) {
        let df = if let Some(primary) = primary {
            Self::fetch_components_pov(store, timeline_query, ent_path, primary, components)
        } else {
            let series = components
                .iter()
                .filter_map(|&component| {
                    Self::fetch_component_pov(store, timeline_query, ent_path, component, component)
                })
                .collect::<Vec<_>>();

            DataFrame::new(series).unwrap()
        };

        let series = expected
            .into_iter()
            .filter_map(|(name, time, idx)| {
                self.all_data
                    .get(&(name.to_owned(), time))
                    .and_then(|entries| entries.get(idx).cloned())
                    .map(|data| (name, data))
            })
            .map(|(name, data)| Series::try_from((name, data)).unwrap())
            .collect::<Vec<_>>();
        let expected = DataFrame::new(series).unwrap();
        let expected = expected.explode(expected.get_column_names()).unwrap();

        store.sort_indices();
        assert_eq!(
            expected, df,
            "\nScenario: {scenario}.\nExpected: {expectation}.\n{store}"
        );
    }

    fn fetch_component_pov(
        store: &DataStore,
        timeline_query: &TimelineQuery,
        ent_path: &EntityPath,
        primary: ComponentNameRef<'_>,
        component: ComponentNameRef<'_>,
    ) -> Option<Series> {
        let row_indices = store
            .query(timeline_query, ent_path, primary, &[component])
            .unwrap_or_default();
        let mut results = store.get(&[component], &row_indices);
        std::mem::take(&mut results[0]).map(|row| Series::try_from((component, row)).unwrap())
    }

    fn fetch_components_pov<const N: usize>(
        store: &DataStore,
        timeline_query: &TimelineQuery,
        ent_path: &EntityPath,
        primary: ComponentNameRef<'_>,
        components: &[ComponentNameRef<'_>; N],
    ) -> DataFrame {
        let row_indices = store
            .query(timeline_query, ent_path, primary, components)
            .unwrap_or([None; N]);
        let results = store.get(components, &row_indices);

        let df = {
            let series: Vec<_> = components
                .iter()
                .zip(results)
                .filter_map(|(component, col)| col.map(|col| (component, col)))
                .map(|(&component, col)| Series::try_from((component, col)).unwrap())
                .collect();

            DataFrame::new(series).unwrap()
        };

        df
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

    let mut store_forward = DataStore::new(
        Instance::NAME.to_owned(),
        DataStoreConfig {
            index_bucket_nb_rows: 10,
            ..Default::default()
        },
    );
    let mut store_backward = DataStore::new(
        Instance::NAME.to_owned(),
        DataStoreConfig {
            index_bucket_nb_rows: 10,
            ..Default::default()
        },
    );

    fn store_repeated_frame(
        frame_nr: i64,
        num: usize,
        store_forward: &mut DataStore,
        store_backward: &mut DataStore,
    ) {
        let ent_path = EntityPath::from("this/that");
        let nb_instances = 1;

        let time_point = TimePoint::from([build_frame_nr(frame_nr)]);
        for _ in 0..num {
            let msg = MsgBundle::new(
                MsgId::ZERO,
                ent_path.clone(),
                time_point.clone(),
                vec![build_instances(nb_instances)],
            );
            store_forward.insert(&msg).unwrap();

            let msg = MsgBundle::new(
                MsgId::ZERO,
                ent_path.clone(),
                time_point.clone(),
                vec![build_instances(nb_instances)],
            );
            store_backward.insert(&msg).unwrap();
        }
    }

    fn store_frame_range(
        range: core::ops::RangeInclusive<i64>,
        store_forward: &mut DataStore,
        store_backward: &mut DataStore,
    ) {
        let ent_path = EntityPath::from("this/that");
        let nb_instances = 1;

        let msgs = range
            .map(|frame_nr| {
                let time_point = TimePoint::from([build_frame_nr(frame_nr)]);
                MsgBundle::new(
                    MsgId::ZERO,
                    ent_path.clone(),
                    time_point,
                    vec![build_instances(nb_instances)],
                )
            })
            .collect::<Vec<_>>();

        msgs.iter()
            .for_each(|msg| store_forward.insert(msg).unwrap());

        msgs.iter()
            .rev()
            .for_each(|msg| store_backward.insert(msg).unwrap());
    }

    store_repeated_frame(1000, 10, &mut store_forward, &mut store_backward);
    store_frame_range(970..=979, &mut store_forward, &mut store_backward);
    store_frame_range(990..=999, &mut store_forward, &mut store_backward);
    store_frame_range(980..=989, &mut store_forward, &mut store_backward);
    store_repeated_frame(1000, 7, &mut store_forward, &mut store_backward);
    store_frame_range(1000..=1009, &mut store_forward, &mut store_backward);
    store_repeated_frame(975, 10, &mut store_forward, &mut store_backward);

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
