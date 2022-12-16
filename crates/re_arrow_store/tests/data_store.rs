use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering::SeqCst},
};

use arrow2::array::Array;

use polars_core::{prelude::DataFrame, series::Series};
use re_arrow_store::{DataStore, DataStoreConfig, LatestAtQuery, RangeQuery, TimeInt, TimeRange};
use re_log_types::{
    datagen::{
        build_frame_nr, build_instances, build_log_time, build_some_point2d, build_some_rects,
    },
    field_types::{Point2D, Rect2D},
    msg_bundle::{Component, ComponentBundle, MsgBundle},
    ComponentName, ComponentNameRef, Duration, MsgId, ObjPath as EntityPath, Time, TimeType,
    Timeline,
};

// --- Configs ---

const COMPONENT_CONFIGS: &[DataStoreConfig] = &[
    // DataStoreConfig::DEFAULT,
    DataStoreConfig {
        component_bucket_nb_rows: 0,
        ..DataStoreConfig::DEFAULT
    },
    // DataStoreConfig {
    //     component_bucket_nb_rows: 1,
    //     ..DataStoreConfig::DEFAULT
    // },
    // DataStoreConfig {
    //     component_bucket_nb_rows: 2,
    //     ..DataStoreConfig::DEFAULT
    // },
    // DataStoreConfig {
    //     component_bucket_nb_rows: 3,
    //     ..DataStoreConfig::DEFAULT
    // },
    // DataStoreConfig {
    //     component_bucket_size_bytes: 0,
    //     ..DataStoreConfig::DEFAULT
    // },
    // DataStoreConfig {
    //     component_bucket_size_bytes: 16,
    //     ..DataStoreConfig::DEFAULT
    // },
    // DataStoreConfig {
    //     component_bucket_size_bytes: 32,
    //     ..DataStoreConfig::DEFAULT
    // },
    // DataStoreConfig {
    //     component_bucket_size_bytes: 64,
    //     ..DataStoreConfig::DEFAULT
    // },
];

const INDEX_CONFIGS: &[DataStoreConfig] = &[
    // DataStoreConfig::DEFAULT,
    DataStoreConfig {
        index_bucket_nb_rows: 0,
        ..DataStoreConfig::DEFAULT
    },
    // DataStoreConfig {
    //     index_bucket_nb_rows: 1,
    //     ..DataStoreConfig::DEFAULT
    // },
    // DataStoreConfig {
    //     index_bucket_nb_rows: 2,
    //     ..DataStoreConfig::DEFAULT
    // },
    // DataStoreConfig {
    //     index_bucket_nb_rows: 3,
    //     ..DataStoreConfig::DEFAULT
    // },
    // DataStoreConfig {
    //     index_bucket_size_bytes: 0,
    //     ..DataStoreConfig::DEFAULT
    // },
    // DataStoreConfig {
    //     index_bucket_size_bytes: 16,
    //     ..DataStoreConfig::DEFAULT
    // },
    // DataStoreConfig {
    //     index_bucket_size_bytes: 32,
    //     ..DataStoreConfig::DEFAULT
    // },
    // DataStoreConfig {
    //     index_bucket_size_bytes: 64,
    //     ..DataStoreConfig::DEFAULT
    // },
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

// --- Scenarios / LatestAt ---

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

/// Covering a very common end-to-end use case:
/// - single entity path
/// - static set of instances
/// - multiple components uploaded at different rates
/// - multiple timelines with non-monotically increasing updates
/// - no weird stuff (duplicated components etc)
#[test]
fn latest_at_standard() {
    init_logs();

    for config in all_configs() {
        let mut store = DataStore::new(config.clone());
        latest_at_standard_impl(&mut store);
    }
}
fn latest_at_standard_impl(store: &mut DataStore) {
    let ent_path = EntityPath::from("this/that");

    let now = Time::now();
    let now_nanos = now.nanos_since_epoch().into();
    let now_minus_2s = now - Duration::from_secs(2.0);
    let now_minus_2s_nanos = now_minus_2s.nanos_since_epoch().into();
    let now_minus_1s = now - Duration::from_secs(1.0);
    let now_minus_1s_nanos = now_minus_1s.nanos_since_epoch().into();
    let now_plus_1s = now + Duration::from_secs(1.0);
    let now_plus_1s_nanos = now_plus_1s.nanos_since_epoch().into();
    let now_plus_2s = now + Duration::from_secs(2.0);
    let now_plus_2s_nanos = now_plus_2s.nanos_since_epoch().into();

    let frame40: TimeInt = 40.into();
    let frame41: TimeInt = 41.into();
    let frame42: TimeInt = 42.into();
    let frame43: TimeInt = 43.into();
    let frame44: TimeInt = 44.into();

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

    store.sort_indices();
    eprintln!("{store}");
    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
    }

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_log_time = Timeline::new("log_time", TimeType::Time);
    let components_all = &["instances", Rect2D::NAME, Point2D::NAME];

    let scenarios = [
        // --- LatestAt + unit-length RangeAt at all frames ---
        (
            "query all components at frame #40 (i.e. before first frame)",
            "empy dataframe",
            (timeline_frame_nr, frame40),
            vec![],
        ),
        (
            "query all components at frame #41 (i.e. first frame with data)",
            "data at that point in time",
            (timeline_frame_nr, frame41),
            vec![
                ("instances", frame41),
                (Rect2D::NAME, frame41),
                (Point2D::NAME, frame41),
            ],
        ),
        (
            "query all components at frame #42 (i.e. second frame with data)",
            "data at that point in time",
            (timeline_frame_nr, frame42),
            vec![
                ("instances", frame42),
                (Rect2D::NAME, frame42),
                (Point2D::NAME, frame42),
            ],
        ),
        (
            "query all components at frame #43 (i.e. last frame with data)",
            "latest data for all components",
            (timeline_frame_nr, frame43),
            vec![
                ("instances", frame42),
                (Rect2D::NAME, frame43),
                (Point2D::NAME, frame42),
            ],
        ),
        (
            "query all components at frame #44 (i.e. after last frame)",
            "latest data for all components",
            (timeline_frame_nr, frame44),
            vec![
                ("instances", frame42),
                (Rect2D::NAME, frame43),
                (Point2D::NAME, frame44),
            ],
        ),
        // --- LatestAt + unit-length RangeAt at all times ---
        (
            "query all components at -2s (i.e. before first update)",
            "empty dataframe",
            (timeline_log_time, now_minus_2s_nanos),
            vec![],
        ),
        (
            "query all components at -1s (i.e. first update)",
            "data at that point in time",
            (timeline_log_time, now_minus_1s_nanos),
            vec![
                (Rect2D::NAME, now_minus_1s_nanos),
                (Point2D::NAME, now_minus_1s_nanos),
            ],
        ),
        (
            "query all components at 0s (i.e. second update)",
            "data at that point in time",
            (timeline_log_time, now_nanos),
            vec![
                ("instances", now_nanos),
                (Rect2D::NAME, now_nanos),
                (Point2D::NAME, now_minus_1s_nanos),
            ],
        ),
        (
            "query all components at +1s (i.e. last update)",
            "latest data for all components",
            (timeline_log_time, now_plus_1s_nanos),
            vec![
                ("instances", now_plus_1s_nanos),
                (Rect2D::NAME, now_plus_1s_nanos),
                (Point2D::NAME, now_minus_1s_nanos),
            ],
        ),
        (
            "query all components at +2s (i.e. after last update)",
            "latest data for all components",
            (timeline_log_time, now_plus_2s_nanos),
            vec![
                ("instances", now_plus_1s_nanos),
                (Rect2D::NAME, now_plus_1s_nanos),
                (Point2D::NAME, now_minus_1s_nanos),
            ],
        ),
    ];

    for (scenario, expectation, (timeline, time), expected) in scenarios {
        // latest_at
        tracker.assert_latest_at(
            scenario,
            expectation,
            store,
            &LatestAtQuery::new(timeline, time),
            &ent_path,
            components_all,
            expected.clone(),
        );

        // // range
        // let time = (time.as_i64() + 1).into();
        // let expected = expected
        //     .into_iter()
        //     .map(|(component, time)| (component, vec![time]))
        //     .collect();
        // tracker.assert_range(
        //     scenario,
        //     expectation,
        //     store,
        //     &RangeQuery::new(timeline, TimeRange::new(time, time)),
        //     &ent_path,
        //     components_all,
        //     expected,
        // );
    }
}

#[test]
fn latest_at_pov() {
    init_logs();

    for config in all_configs() {
        let mut store = DataStore::new(config.clone());
        latest_at_pov_impl(&mut store);
    }
}
fn latest_at_pov_impl(store: &mut DataStore) {
    let ent_path = EntityPath::from("this/that");

    let frame40: TimeInt = 40.into();
    let frame41: TimeInt = 41.into();
    let frame42: TimeInt = 42.into();

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
    let components_all = &["instances", Rect2D::NAME, Point2D::NAME];

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
            "instances",
            vec![],
        ),
        (
            "query all components at frame #41, from `rects` PoV",
            "the set of `rects` and the _first_ set of `instances` at that time",
            frame41,
            Rect2D::NAME,
            vec![("instances", frame41, 0), (Rect2D::NAME, frame41, 0)],
        ),
        (
            "query all components at frame #41, from `positions` PoV",
            "the set of `positions` and the _second_ set of `instances` at that time",
            frame41,
            Point2D::NAME,
            vec![("instances", frame41, 1), (Point2D::NAME, frame41, 0)],
        ),
        (
            "query all components at frame #41, from `instances` PoV",
            "the _second_ set of `instances` and the set of `positions` at that time",
            frame41,
            "instances",
            vec![("instances", frame41, 1), (Point2D::NAME, frame41, 0)],
        ),
        (
            "query all components at frame #42, from `positions` PoV",
            "the set of `positions` and the set of `instances` at that time",
            frame42,
            Point2D::NAME,
            vec![("instances", frame42, 0), (Point2D::NAME, frame42, 0)],
        ),
        (
            "query all components at frame #42, from `rects` PoV",
            "the set of `rects` at that time",
            frame42,
            Rect2D::NAME,
            vec![(Rect2D::NAME, frame42, 0)],
        ),
        (
            "query all components at frame #42, from `instances` PoV",
            "the set of `positions` and the set of `instances` at that time",
            frame42,
            "instances",
            vec![("instances", frame42, 0), (Point2D::NAME, frame42, 0)],
        ),
    ];

    for (scenario, expectation, time, primary, expected) in scenarios {
        // latest_at
        tracker.assert_latest_at_pov(
            scenario,
            expectation,
            store,
            &LatestAtQuery::new(timeline_frame_nr, time),
            &ent_path,
            primary,
            components_all,
            expected.clone(),
        );

        // // range
        // let time = (time.as_i64() + 1).into();
        // let expected = expected
        //     .into_iter()
        //     .map(|(component, time, idx)| (component, vec![time], idx))
        //     .collect();
        // tracker.assert_range_pov(
        //     scenario,
        //     expectation,
        //     store,
        //     &RangeQuery::new(timeline_frame_nr, TimeRange::new(time, time)),
        //     &ent_path,
        //     primary,
        //     components_all,
        //     expected,
        // );
    }
}

#[test]
fn latest_at_emptiness_edge_cases() {
    init_logs();

    for config in all_configs() {
        let mut store = DataStore::new(config.clone());
        latest_at_emptiness_edge_cases_impl(&mut store);
    }
}
fn latest_at_emptiness_edge_cases_impl(store: &mut DataStore) {
    let ent_path = EntityPath::from("this/that");
    let now = Time::now();
    let now_nanos = now.nanos_since_epoch();
    let now_minus_1s = now - Duration::from_secs(1.0);
    let now_minus_1s_nanos = now_minus_1s.nanos_since_epoch();
    let frame39: TimeInt = 39.into();
    let frame40: TimeInt = 40.into();
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
    let components_all = &["instances"];

    tracker.assert_latest_at(
        "query at `last_frame`",
        "dataframe with our instances in it",
        store,
        &LatestAtQuery::new(timeline_frame_nr, frame40),
        &ent_path,
        components_all,
        vec![("instances", frame40)],
    );

    tracker.assert_latest_at(
        "query at `last_log_time`",
        "dataframe with our instances in it",
        store,
        &LatestAtQuery::new(timeline_log_time, now_nanos.into()),
        &ent_path,
        components_all,
        vec![("instances", now_nanos.into())],
    );

    tracker.assert_latest_at(
        "query an empty store at `first_frame - 1`",
        "empty dataframe",
        store,
        &LatestAtQuery::new(timeline_frame_nr, frame39),
        &ent_path,
        components_all,
        vec![],
    );

    tracker.assert_latest_at(
        "query an empty store at `first_log_time - 1s`",
        "empty dataframe",
        store,
        &LatestAtQuery::new(timeline_log_time, now_minus_1s_nanos.into()),
        &ent_path,
        components_all,
        vec![],
    );

    tracker.assert_latest_at(
        "query a non-existing entity path",
        "empty dataframe",
        store,
        &LatestAtQuery::new(timeline_frame_nr, frame40),
        &EntityPath::from("does/not/exist"),
        components_all,
        vec![],
    );

    tracker.assert_latest_at(
        "query a bunch of non-existing components",
        "empty dataframe",
        store,
        &LatestAtQuery::new(timeline_frame_nr, frame40),
        &ent_path,
        &["they", "dont", "exist"],
        vec![],
    );

    tracker.assert_latest_at(
        "query with an empty list of components",
        "empty dataframe",
        store,
        &LatestAtQuery::new(timeline_frame_nr, frame40),
        &ent_path,
        &[],
        vec![],
    );

    tracker.assert_latest_at(
        "query with wrong timeline name",
        "empty dataframe",
        store,
        &LatestAtQuery::new(timeline_wrong_name, frame40),
        &ent_path,
        components_all,
        vec![],
    );

    tracker.assert_latest_at(
        "query with wrong timeline kind",
        "empty dataframe",
        store,
        &LatestAtQuery::new(timeline_wrong_kind, frame40),
        &ent_path,
        components_all,
        vec![],
    );
}

// --- Scenarios / Range ---

// TODO:
// - range needs to return all entries in a single frame yo

#[test]
fn range_standard() {
    init_logs();

    for config in all_configs() {
        let mut store = DataStore::new(config.clone());
        range_standard_impl(&mut store);
    }
}
fn range_standard_impl(store: &mut DataStore) {
    let ent_path = EntityPath::from("this/that");

    // TODO: range emptiness edge cases

    let now = Time::now();
    let now_nanos = now.nanos_since_epoch();
    let now_minus_2s = now - Duration::from_secs(2.0);
    let now_minus_1s = now - Duration::from_secs(1.0);
    let now_minus_1s_nanos = now_minus_1s.nanos_since_epoch();
    let now_plus_1s = now + Duration::from_secs(1.0);
    let now_plus_1s_nanos = now_plus_1s.nanos_since_epoch();
    let now_plus_2s = now + Duration::from_secs(2.0);

    let frame40: TimeInt = 40.into();
    let frame41: TimeInt = 41.into();
    let frame42: TimeInt = 42.into();
    let frame43: TimeInt = 43.into();
    let frame44: TimeInt = 44.into();

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
                build_instances(nb_instances),
            ]),
        );
    }

    store.sort_indices();
    eprintln!("{store}");

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
    }

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_log_time = Timeline::new("log_time", TimeType::Time);
    let components_all = &["instances", Rect2D::NAME, Point2D::NAME];

    // --- Testing at all frames ---

    let scenarios = [
        (
            "query all components at frame #40 (i.e. before first frame)",
            "empy dataframe",
            frame44,
            vec![],
        ),
        //
    ];

    for (scenario, expectation, frame_nr, expected) in scenarios {
        tracker.assert_range(
            scenario,
            expectation,
            store,
            &RangeQuery::new(timeline_frame_nr, TimeRange::new(0.into(), frame_nr)),
            &ent_path,
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

    /// Asserts a simple `latest_at` scenario, where every component is fetched from its own
    /// point-of-view.
    #[allow(clippy::too_many_arguments)]
    fn assert_latest_at<const N: usize>(
        &self,
        scenario: &str,
        expectation: &str,
        store: &mut DataStore,
        query: &LatestAtQuery,
        ent_path: &EntityPath,
        components: &[ComponentNameRef<'_>; N],
        expected: Vec<(ComponentNameRef<'static>, TimeInt)>,
    ) {
        self.assert_latest_at_pov_impl(
            scenario,
            expectation,
            store,
            query,
            ent_path,
            None,
            components,
            expected
                .into_iter()
                .map(|(name, time)| (name, time, 0))
                .collect(),
        );
    }

    /// Asserts a pov `latest_at` scenario, where every component is fetched as it is seen from
    /// the point-of-view of another component.
    #[allow(clippy::too_many_arguments)]
    fn assert_latest_at_pov<const N: usize>(
        &self,
        scenario: &str,
        expectation: &str,
        store: &mut DataStore,
        query: &LatestAtQuery,
        ent_path: &EntityPath,
        primary: ComponentNameRef<'_>,
        components: &[ComponentNameRef<'_>; N],
        expected: Vec<(ComponentNameRef<'static>, TimeInt, usize)>,
    ) {
        self.assert_latest_at_pov_impl(
            scenario,
            expectation,
            store,
            query,
            ent_path,
            primary.into(),
            components,
            expected,
        );
    }

    /// Asserts a complex `latest_at` scenario, where every component is either fetched as it is
    /// seen from the point-of-view of another component, or its own point-of-view if primary
    /// is None.
    #[allow(clippy::too_many_arguments)]
    fn assert_latest_at_pov_impl<const N: usize>(
        &self,
        scenario: &str,
        expectation: &str,
        store: &mut DataStore,
        query: &LatestAtQuery,
        ent_path: &EntityPath,
        primary: Option<ComponentNameRef<'_>>,
        components: &[ComponentNameRef<'_>; N],
        expected: Vec<(ComponentNameRef<'static>, TimeInt, usize)>,
    ) {
        fn fetch_component_pov(
            store: &DataStore,
            query: &LatestAtQuery,
            ent_path: &EntityPath,
            primary: ComponentNameRef<'_>,
            component: ComponentNameRef<'_>,
        ) -> Option<Series> {
            let row_indices = store
                .latest_at(query, ent_path, primary, &[component])
                .unwrap_or_default();
            let mut results = store.get(&[component], &row_indices);
            std::mem::take(&mut results[0]).map(|row| Series::try_from((component, row)).unwrap())
        }

        fn fetch_components_pov<const N: usize>(
            store: &DataStore,
            query: &LatestAtQuery,
            ent_path: &EntityPath,
            primary: ComponentNameRef<'_>,
            components: &[ComponentNameRef<'_>; N],
        ) -> DataFrame {
            let row_indices = store
                .latest_at(query, ent_path, primary, components)
                .unwrap_or([None; N]);
            let results = store.get(components, &row_indices);

            let df = {
                let series: Vec<_> = components
                    .iter()
                    .zip(results)
                    .filter_map(|(component, col)| col.map(|col| (component, col)))
                    .map(|(&component, col)| Series::try_from((component, col)).unwrap())
                    .collect();

                let df = DataFrame::new(series).unwrap();
                df.explode(df.get_column_names()).unwrap()
            };

            df
        }

        let df = if let Some(primary) = primary {
            fetch_components_pov(store, query, ent_path, primary, components)
        } else {
            let series = components
                .iter()
                .map(|&component| {
                    fetch_components_pov(store, query, ent_path, component, components)
                })
                .collect::<Vec<_>>();

            let df = polars_core::functions::hor_concat_df(dbg!(&series)).unwrap();
            // let mut df = DataFrame::empty();
            // for xxx in series {
            //     dbg!(&xxx);
            //     df.vstack_mut(&xxx).unwrap();
            // }
            df
            // let df = DataFrame::new(series).unwrap();
            // df.explode(df.get_column_names()).unwrap()
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

    // TODO
    #[allow(clippy::too_many_arguments)]
    fn assert_range<const N: usize>(
        &self,
        scenario: &str,
        expectation: &str,
        store: &mut DataStore,
        query: &RangeQuery,
        ent_path: &EntityPath,
        components: &[ComponentNameRef<'_>; N],
        expected: Vec<(ComponentNameRef<'static>, Vec<TimeInt>)>,
    ) {
        self.assert_range_pov_impl(
            scenario,
            expectation,
            store,
            query,
            ent_path,
            None,
            components,
            expected
                .into_iter()
                .map(|(name, times)| (name, times, 0))
                .collect(),
        );
    }

    // TODO
    #[allow(clippy::too_many_arguments)]
    fn assert_range_pov<const N: usize>(
        &self,
        scenario: &str,
        expectation: &str,
        store: &mut DataStore,
        query: &RangeQuery,
        ent_path: &EntityPath,
        primary: ComponentNameRef<'_>,
        components: &[ComponentNameRef<'_>; N],
        expected: Vec<(ComponentNameRef<'static>, Vec<TimeInt>)>,
    ) {
        self.assert_range_pov_impl(
            scenario,
            expectation,
            store,
            query,
            ent_path,
            primary.into(),
            components,
            expected
                .into_iter()
                .map(|(name, times)| (name, times, 0))
                .collect(),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn assert_range_pov_impl<const N: usize>(
        &self,
        scenario: &str,
        expectation: &str,
        store: &mut DataStore,
        query: &RangeQuery,
        ent_path: &EntityPath,
        primary: Option<ComponentNameRef<'_>>,
        components: &[ComponentNameRef<'_>; N],
        expected: Vec<(ComponentNameRef<'static>, Vec<TimeInt>, usize)>,
    ) {
        fn fetch_component_pov(
            store: &DataStore,
            query: &RangeQuery,
            ent_path: &EntityPath,
            primary: ComponentNameRef<'_>,
            component: ComponentNameRef<'_>,
        ) -> Option<Series> {
            let components = &[component];
            let row_indices = store.range(query, ent_path, primary, components);
            let rows = row_indices
                .map(|(_, row_indices)| store.get(&[component], &row_indices))
                .filter_map(|mut results| {
                    std::mem::take(&mut results[0])
                        .map(|row| Series::try_from((component, row)).unwrap())
                })
                .collect::<Vec<_>>();
            // dbg!(&rows);

            // TODO: maybe we ask the store what type we're expecting here...?

            (!rows.is_empty()).then(|| {
                let mut series = Series::new_empty(component, rows[0].dtype());
                for row in rows {
                    series.append(&row).unwrap(); // TODO
                }
                dbg!(series)
            })
        }

        // fn fetch_components_pov(
        //     store: &DataStore,
        //     query: &RangeQuery,
        //     ent_path: &EntityPath,
        //     primary: ComponentNameRef<'_>,
        //     component: ComponentNameRef<'_>,
        // ) -> DataFrame {
        //     let components = &[component];
        //     let row_indices = store.range(query, ent_path, primary, components);
        //     let rows = row_indices
        //         .map(|(_, row_indices)| store.get(&[component], &row_indices))
        //         .filter_map(|mut results| {
        //             std::mem::take(&mut results[0])
        //                 .map(|row| Series::try_from((component, row)).unwrap())
        //         })
        //         .collect::<Vec<_>>();
        //     // dbg!(&rows);

        //     // TODO: maybe we ask the store what type we're expecting here...?

        //     (!rows.is_empty()).then(|| {
        //         let mut series = Series::new_empty(component, rows[0].dtype());
        //         for row in rows {
        //             series.append(&row).unwrap(); // TODO
        //         }
        //         dbg!(series)
        //     })
        // }

        // fn fetch_components_pov<const N: usize>(
        //     store: &DataStore,
        //     query: &RangeQuery,
        //     ent_path: &EntityPath,
        //     primary: ComponentNameRef<'_>,
        //     components: &[ComponentNameRef<'_>; N],
        // ) -> DataFrame {
        //     let row_indices = store.range(query, ent_path, primary, components);
        //     let rows = row_indices
        //         .map(|(_, row_indices)| store.get(components, &row_indices))
        //         .filter_map(|mut results| {
        //             std::mem::take(&mut results[0])
        //                 .map(|row| Series::try_from((component, row)).unwrap())
        //         })
        //         .collect::<Vec<_>>();

        //     let df = {
        //         let series: Vec<_> = components
        //             .iter()
        //             .zip(results)
        //             .filter_map(|(component, col)| col.map(|col| (component, col)))
        //             .map(|(&component, col)| Series::try_from((component, col)).unwrap())
        //             .collect();

        //         let df = DataFrame::new(series).unwrap();
        //         df.explode(df.get_column_names()).unwrap()
        //     };

        //     df
        // }

        let df = if let Some(primary) = primary {
            todo!()
            // fetch_components_pov(store, query, ent_path, primary, components)
        } else {
            let series = components
                .iter()
                .filter_map(|&component| {
                    fetch_component_pov(store, query, ent_path, component, component)
                })
                .collect::<Vec<_>>();

            dbg!(&series);

            let df = DataFrame::new(series).unwrap();
            df.explode(df.get_column_names()).unwrap()
        };

        let series = expected
            .into_iter()
            .flat_map(|(name, times, idx)| {
                times.into_iter().filter_map(move |time| {
                    self.all_data
                        .get(&(name.to_owned(), time))
                        .and_then(|entries| entries.get(idx).cloned())
                        .map(|data| (name, data))
                })
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
}

fn init_logs() {
    static INIT: AtomicBool = AtomicBool::new(false);

    if INIT.compare_exchange(false, true, SeqCst, SeqCst).is_ok() {
        re_log::set_default_rust_log_env();
        tracing_subscriber::fmt::init(); // log to stdout
    }
}
