use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering::SeqCst},
};

use arrow2::array::{Array, UInt64Array};

use polars_core::{prelude::DataFrame, series::Series};
use re_arrow_store::{DataStore, DataStoreConfig, TimeInt, TimeQuery, TimelineQuery, WriteError};
use re_log_types::{
    datagen::{
        build_frame_nr, build_instances, build_log_time, build_some_point2d, build_some_rects,
    },
    field_types::{Instance, Point2D, Rect2D},
    msg_bundle::{wrap_in_listarray, Component as _, ComponentBundle, MsgBundle},
    ComponentName, Duration, MsgId, ObjPath as EntityPath, Time, TimeType, Timeline,
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
        let mut store = DataStore::new(Instance::name(), config.clone());
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
    let components_all = &[Instance::name()];

    tracker.assert_scenario(
        "query at `last_frame`",
        "dataframe with our instances in it",
        store,
        &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame40)),
        &ent_path,
        components_all,
        vec![(Instance::name(), frame40.into())],
    );

    tracker.assert_scenario(
        "query at `last_log_time`",
        "dataframe with our instances in it",
        store,
        &TimelineQuery::new(timeline_log_time, TimeQuery::LatestAt(now_nanos)),
        &ent_path,
        components_all,
        vec![(Instance::name(), now_nanos.into())],
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
        &["they".into(), "dont".into(), "exist".into()],
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
        let mut store = DataStore::new(Instance::name(), config.clone());
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
    let components_all = &[Instance::name(), Rect2D::name(), Point2D::name()];

    // --- Testing at all frames ---

    // TODO: I am not quite sure why this works...?

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
                (Instance::name(), frame41.into()),
                (Rect2D::name(), frame41.into()),
                (Point2D::name(), frame41.into()),
            ],
        ),
        (
            "query all components at frame #42 (i.e. second frame with data)",
            "data at that point in time",
            frame42,
            vec![
                (Instance::name(), frame42.into()),
                (Rect2D::name(), frame42.into()),
                (Point2D::name(), frame42.into()),
            ],
        ),
        (
            "query all components at frame #43 (i.e. last frame with data)",
            "latest data for all components",
            frame43,
            vec![
                (Instance::name(), frame42.into()),
                (Rect2D::name(), frame43.into()),
                (Point2D::name(), frame42.into()),
            ],
        ),
        (
            "query all components at frame #44 (i.e. after last frame)",
            "latest data for all components",
            frame44,
            vec![
                (Instance::name(), frame42.into()),
                (Rect2D::name(), frame43.into()),
                (Point2D::name(), frame44.into()),
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
                (Rect2D::name(), now_minus_1s_nanos.into()),
                (Point2D::name(), now_minus_1s_nanos.into()),
            ],
        ),
        (
            "query all components at 0s (i.e. second update)",
            "data at that point in time",
            now,
            vec![
                (Instance::name(), now_nanos.into()),
                (Rect2D::name(), now_nanos.into()),
                (Point2D::name(), now_minus_1s_nanos.into()),
            ],
        ),
        (
            "query all components at +1s (i.e. last update)",
            "latest data for all components",
            now_plus_1s,
            vec![
                (Instance::name(), now_plus_1s_nanos.into()),
                (Rect2D::name(), now_plus_1s_nanos.into()),
                (Point2D::name(), now_minus_1s_nanos.into()),
            ],
        ),
        (
            "query all components at +2s (i.e. after last update)",
            "latest data for all components",
            now_plus_2s,
            vec![
                (Instance::name(), now_plus_1s_nanos.into()),
                (Rect2D::name(), now_plus_1s_nanos.into()),
                (Point2D::name(), now_minus_1s_nanos.into()),
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
        let mut store = DataStore::new(Instance::name(), config.clone());
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
    let components_all = &[Instance::name(), Rect2D::name(), Point2D::name()];

    let scenarios = [
        (
            "query all components at frame #40, from `rects` PoV",
            "empty dataframe",
            frame40,
            Rect2D::name(),
            vec![],
        ),
        (
            "query all components at frame #40, from `positions` PoV",
            "empty dataframe",
            frame40,
            Point2D::name(),
            vec![],
        ),
        (
            "query all components at frame #40, from `instances` PoV",
            "empty dataframe",
            frame40,
            Instance::name(),
            vec![],
        ),
        (
            "query all components at frame #41, from `rects` PoV",
            "the set of `rects` and the _first_ set of `instances` at that time",
            frame41,
            Rect2D::name(),
            vec![
                (Instance::name(), frame41.into(), 0),
                (Rect2D::name(), frame41.into(), 0),
            ],
        ),
        (
            "query all components at frame #41, from `positions` PoV",
            "the set of `positions` and the _second_ set of `instances` at that time",
            frame41,
            Point2D::name(),
            vec![
                (Instance::name(), frame41.into(), 1),
                (Point2D::name(), frame41.into(), 0),
            ],
        ),
        (
            "query all components at frame #41, from `instances` PoV",
            "the _second_ set of `instances` and the set of `positions` at that time",
            frame41,
            Instance::name(),
            vec![
                (Instance::name(), frame41.into(), 1),
                (Point2D::name(), frame41.into(), 0),
            ],
        ),
        (
            "query all components at frame #42, from `positions` PoV",
            "the set of `positions` and the set of `instances` at that time",
            frame42,
            Point2D::name(),
            vec![
                (Instance::name(), frame42.into(), 0),
                (Point2D::name(), frame42.into(), 0),
            ],
        ),
        (
            "query all components at frame #42, from `rects` PoV",
            "the set of `rects` at that time",
            frame42,
            Rect2D::name(),
            vec![(Rect2D::name(), frame42.into(), 0)],
        ),
        (
            "query all components at frame #42, from `instances` PoV",
            "the set of `positions` and the set of `instances` at that time",
            frame42,
            Instance::name(),
            vec![
                (Instance::name(), frame42.into(), 0),
                (Point2D::name(), frame42.into(), 0),
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

#[test]
fn write_errors() {
    {
        use arrow2::compute::concatenate::concatenate;

        let mut store = DataStore::new(Instance::name(), Default::default());
        let mut bundle = re_log_types::msg_bundle::try_build_msg_bundle2(
            MsgId::ZERO,
            EntityPath::from("this/that"),
            [build_frame_nr(32), build_log_time(Time::now())],
            (build_instances(10), build_some_point2d(10)),
        )
        .unwrap();

        // make instances 2 rows long
        bundle.components[0].value =
            concatenate(&[&*bundle.components[0].value, &*bundle.components[0].value]).unwrap();

        assert!(matches!(
            store.insert(&bundle),
            Err(WriteError::BadBatchLength(_)),
        ));
    }

    {
        use arrow2::compute::concatenate::concatenate;

        let mut store = DataStore::new(Instance::name(), Default::default());
        let mut bundle = re_log_types::msg_bundle::try_build_msg_bundle2(
            MsgId::ZERO,
            EntityPath::from("this/that"),
            [build_frame_nr(32), build_log_time(Time::now())],
            (build_instances(10), build_some_point2d(10)),
        )
        .unwrap();

        // make instances 2 rows long
        bundle.components[1].value =
            concatenate(&[&*bundle.components[1].value, &*bundle.components[1].value]).unwrap();

        assert!(matches!(
            store.insert(&bundle),
            Err(WriteError::MismatchedRows(_)),
        ));
    }

    {
        pub fn build_sparse_instances() -> ComponentBundle {
            let ids = wrap_in_listarray(UInt64Array::from(vec![Some(1), None, Some(3)]).boxed());
            ComponentBundle {
                name: Instance::name(),
                value: ids.boxed(),
            }
        }

        let mut store = DataStore::new(Instance::name(), Default::default());
        let bundle = re_log_types::msg_bundle::try_build_msg_bundle2(
            MsgId::ZERO,
            EntityPath::from("this/that"),
            [build_frame_nr(32), build_log_time(Time::now())],
            (build_sparse_instances(), build_some_point2d(3)),
        )
        .unwrap();

        assert!(matches!(
            store.insert(&bundle),
            Err(WriteError::SparseClusteringComponent(_)),
        ));
    }

    {
        pub fn build_unsorted_instances() -> ComponentBundle {
            let ids = wrap_in_listarray(UInt64Array::from_vec(vec![1, 3, 2]).boxed());
            ComponentBundle {
                name: Instance::name(),
                value: ids.boxed(),
            }
        }
        pub fn build_duped_instances() -> ComponentBundle {
            let ids = wrap_in_listarray(UInt64Array::from_vec(vec![1, 2, 2]).boxed());
            ComponentBundle {
                name: Instance::name(),
                value: ids.boxed(),
            }
        }

        let mut store = DataStore::new(Instance::name(), Default::default());
        {
            let bundle = re_log_types::msg_bundle::try_build_msg_bundle2(
                MsgId::ZERO,
                EntityPath::from("this/that"),
                [build_frame_nr(32), build_log_time(Time::now())],
                (build_unsorted_instances(), build_some_point2d(3)),
            )
            .unwrap();
            assert!(matches!(
                store.insert(&bundle),
                Err(WriteError::InvalidClusteringComponent(_)),
            ));
        }
        {
            let bundle = re_log_types::msg_bundle::try_build_msg_bundle2(
                MsgId::ZERO,
                EntityPath::from("this/that"),
                [build_frame_nr(32), build_log_time(Time::now())],
                (build_duped_instances(), build_some_point2d(3)),
            )
            .unwrap();
            assert!(matches!(
                store.insert(&bundle),
                Err(WriteError::InvalidClusteringComponent(_)),
            ));
        }
    }

    {
        let mut store = DataStore::new(Instance::name(), Default::default());
        let bundle = re_log_types::msg_bundle::try_build_msg_bundle2(
            MsgId::ZERO,
            EntityPath::from("this/that"),
            [build_frame_nr(32), build_log_time(Time::now())],
            (build_instances(4), build_some_point2d(3)),
        )
        .unwrap();

        assert!(matches!(
            store.insert(&bundle),
            Err(WriteError::MismatchedInstances { .. }),
        ));
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
                let comps = self.all_data.entry((*name, *time)).or_default();
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
        components: &[ComponentName; N],
        expected: Vec<(ComponentName, TimeInt)>,
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
        primary: ComponentName,
        components: &[ComponentName; N],
        expected: Vec<(ComponentName, TimeInt, usize)>,
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
        primary: Option<ComponentName>,
        components: &[ComponentName; N],
        expected: Vec<(ComponentName, TimeInt, usize)>,
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
                    .get(&(name, time))
                    .and_then(|entries| entries.get(idx).cloned())
                    .map(|data| (name, data))
            })
            .map(|(name, data)| Series::try_from((name.as_str(), data)).unwrap())
            .collect::<Vec<_>>();
        let expected = DataFrame::new(series).unwrap();
        let expected = expected.explode(expected.get_column_names()).unwrap();

        store.sort_indices();
        assert_eq!(
            expected, df,
            "\nScenario: {scenario}.\nExpected: {expectation}.\n{store}"
        );
    }

    // TODO: we can normalize all of this now

    fn fetch_component_pov(
        store: &DataStore,
        timeline_query: &TimelineQuery,
        ent_path: &EntityPath,
        primary: ComponentName,
        component: ComponentName,
    ) -> Option<Series> {
        let row_indices = store
            .query(timeline_query, ent_path, primary, &[component])
            .unwrap_or_default();
        let mut results = store.get(&[component], &row_indices);
        std::mem::take(&mut results[0])
            .map(|row| Series::try_from((component.as_str(), row)).unwrap())
    }

    fn fetch_components_pov<const N: usize>(
        store: &DataStore,
        timeline_query: &TimelineQuery,
        ent_path: &EntityPath,
        primary: ComponentName,
        components: &[ComponentName; N],
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
                .map(|(&component, col)| Series::try_from((component.as_str(), col)).unwrap())
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
