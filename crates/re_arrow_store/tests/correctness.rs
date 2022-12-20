//! Correctness tests.
//!
//! Bending and twisting the datastore APIs in all kinds of weird ways to try and break them.

use std::sync::atomic::{AtomicBool, Ordering::SeqCst};

use arrow2::array::UInt64Array;

use re_arrow_store::{test_bundle, DataStore, TimeQuery, TimelineQuery, WriteError};
use re_log_types::{
    datagen::{build_frame_nr, build_instances, build_log_time, build_some_point2d},
    field_types::Instance,
    msg_bundle::{wrap_in_listarray, Component as _, ComponentBundle},
    Duration, MsgId, ObjPath as EntityPath, Time, TimeType, Timeline,
};

// ---

#[test]
fn write_errors() {
    init_logs();

    let ent_path = EntityPath::from("this/that");

    {
        use arrow2::compute::concatenate::concatenate;

        let mut store = DataStore::new(Instance::name(), Default::default());
        let mut bundle = test_bundle!(ent_path @
            [build_frame_nr(32), build_log_time(Time::now())] => [
                build_instances(10), build_some_point2d(10)
        ]);

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
        let mut bundle = test_bundle!(ent_path @
            [build_frame_nr(32), build_log_time(Time::now())] => [
                build_instances(10), build_some_point2d(10)
        ]);

        // make component 2 rows long
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
        let bundle = test_bundle!(ent_path @
            [build_frame_nr(32), build_log_time(Time::now())] => [
                build_sparse_instances(), build_some_point2d(3)
        ]);
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
            let bundle = test_bundle!(ent_path @
                [build_frame_nr(32), build_log_time(Time::now())] => [
                    build_unsorted_instances(), build_some_point2d(3)
            ]);
            assert!(matches!(
                store.insert(&bundle),
                Err(WriteError::InvalidClusteringComponent(_)),
            ));
        }
        {
            let bundle = test_bundle!(ent_path @
                [build_frame_nr(32), build_log_time(Time::now())] => [
                    build_duped_instances(), build_some_point2d(3)
            ]);
            assert!(matches!(
                store.insert(&bundle),
                Err(WriteError::InvalidClusteringComponent(_)),
            ));
        }
    }

    {
        let mut store = DataStore::new(Instance::name(), Default::default());
        let bundle = test_bundle!(ent_path @
            [build_frame_nr(32), build_log_time(Time::now())] => [
                build_instances(4), build_some_point2d(3)
        ]);
        assert!(matches!(
            store.insert(&bundle),
            Err(WriteError::MismatchedInstances { .. }),
        ));
    }
}

// ---

#[test]
fn empty_query_edge_cases() {
    init_logs();

    for config in re_arrow_store::test_util::all_configs() {
        let mut store = DataStore::new(Instance::name(), config.clone());
        empty_query_edge_cases_impl(&mut store);
    }
}
fn empty_query_edge_cases_impl(store: &mut DataStore) {
    let ent_path = EntityPath::from("this/that");
    let now = Time::now();
    let now_minus_1s = now - Duration::from_secs(1.0);
    let now_minus_1s_nanos = now_minus_1s.nanos_since_epoch();
    let frame39 = 39;
    let frame40 = 40;
    let nb_instances = 3;

    store
        .insert(
            &test_bundle!(ent_path @ [build_log_time(now), build_frame_nr(frame40)] => [
                build_instances(nb_instances),
            ]),
        )
        .unwrap();

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
    }

    let timeline_wrong_name = Timeline::new("lag_time", TimeType::Time);
    let timeline_wrong_kind = Timeline::new("log_time", TimeType::Sequence);
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_log_time = Timeline::new("log_time", TimeType::Time);

    // empty frame_nr
    {
        let row_indices = store.query(
            &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame39)),
            &ent_path,
            Instance::name(),
            &[Instance::name()],
        );
        assert!(row_indices.is_none());
    }

    // empty log_time
    {
        let row_indices = store.query(
            &TimelineQuery::new(timeline_log_time, TimeQuery::LatestAt(now_minus_1s_nanos)),
            &ent_path,
            Instance::name(),
            &[Instance::name()],
        );
        assert!(row_indices.is_none());
    }

    // wrong entity path
    {
        let row_indices = store.query(
            &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame40)),
            &EntityPath::from("does/not/exist"),
            Instance::name(),
            &[Instance::name()],
        );
        assert!(row_indices.is_none());
    }

    // bunch of non-existing components
    {
        let components = &["they".into(), "dont".into(), "exist".into()];
        let row_indices = store
            .query(
                &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame40)),
                &ent_path,
                Instance::name(),
                components,
            )
            .unwrap();
        let rows = store.get(components, &row_indices);
        rows.iter().all(|row| row.is_none());
    }

    // empty component list
    {
        let row_indices = store
            .query(
                &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame40)),
                &ent_path,
                Instance::name(),
                &[],
            )
            .unwrap();
        assert!(row_indices.is_empty());
    }

    // wrong timeline name
    {
        let row_indices = store.query(
            &TimelineQuery::new(timeline_wrong_name, TimeQuery::LatestAt(frame40)),
            &EntityPath::from("does/not/exist"),
            Instance::name(),
            &[Instance::name()],
        );
        assert!(row_indices.is_none());
    }

    // wrong timeline kind
    {
        let row_indices = store.query(
            &TimelineQuery::new(timeline_wrong_kind, TimeQuery::LatestAt(frame40)),
            &EntityPath::from("does/not/exist"),
            Instance::name(),
            &[Instance::name()],
        );
        assert!(row_indices.is_none());
    }
}

// ---

pub fn init_logs() {
    static INIT: AtomicBool = AtomicBool::new(false);

    if INIT.compare_exchange(false, true, SeqCst, SeqCst).is_ok() {
        re_log::set_default_rust_log_env();
        tracing_subscriber::fmt::init(); // log to stdout
    }
}
