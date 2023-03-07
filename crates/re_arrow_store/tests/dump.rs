//! Dumping a datastore to log messages and back.

use std::sync::atomic::{AtomicBool, Ordering};

use arrow2::array::{Array, UInt64Array};
use itertools::Itertools;
use nohash_hasher::IntMap;
use polars_core::{prelude::*, series::Series};
use polars_ops::prelude::DataFrameJoinOps;
use rand::Rng;
use re_arrow_store::{
    polars_util, test_bundle, DataStore, DataStoreConfig, DataStoreStats, GarbageCollectionTarget,
    LatestAtQuery, RangeQuery, TimeInt, TimeRange,
};
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, Point2D, Rect2D},
    datagen::{
        build_frame_nr, build_some_colors, build_some_instances, build_some_instances_from,
        build_some_point2d, build_some_rects,
    },
    external::arrow2_convert::deserialize::arrow_array_deserialize_iterator,
    msg_bundle::{wrap_in_listarray, Component as _, MsgBundle},
    ComponentName, EntityPath, MsgId, TimeType, Timeline,
};

// --- LatestAt ---

#[test]
fn dump() {
    init_logs();

    // config
    //   dump
    //     gc

    for config in re_arrow_store::test_util::all_configs() {
        let mut store1 = DataStore::new(InstanceKey::name(), config.clone());
        let mut store2 = DataStore::new(InstanceKey::name(), config.clone());
        let mut store3 = DataStore::new(InstanceKey::name(), config.clone());

        dump_impl(&mut store1, &mut store2, &mut store3);
        // store1.gc(
        //     GarbageCollectionTarget::DropAtLeastPercentage(1.0),
        //     Timeline::new("frame_nr", TimeType::Sequence),
        //     MsgId::name(),
        // );
        // dump_impl(&mut store1, &mut store2);
    }
}

fn dump_impl(store1: &mut DataStore, store2: &mut DataStore, store3: &mut DataStore) {
    let frame0: TimeInt = 0.into();
    let frame1: TimeInt = 1.into();
    let frame2: TimeInt = 2.into();
    let frame3: TimeInt = 3.into();
    let frame4: TimeInt = 4.into();

    let create_bundles = |store: &mut DataStore, ent_path| {
        let ent_path = EntityPath::from(ent_path);

        let (instances1, colors1) = (build_some_instances(3), build_some_colors(3));
        let bundle1 =
            test_bundle!(ent_path @ [build_frame_nr(frame1)] => [instances1.clone(), colors1]);

        let points2 = build_some_point2d(3);
        let bundle2 = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [instances1, points2]);

        let points3 = build_some_point2d(10);
        let bundle3 = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [points3]);

        let colors4 = build_some_colors(5);
        let bundle4 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [colors4]);

        vec![bundle1, bundle2, bundle3, bundle4]
    };

    // helper to insert a bundle both as a temporal and timeless payload
    let insert = |store: &mut DataStore, bundle: &MsgBundle| {
        // insert temporal
        // TODO
        // store.insert(bundle).unwrap();

        // insert timeless
        let mut bundle_timeless = bundle.clone();
        bundle_timeless.time_point = Default::default();
        store.insert(&bundle_timeless).unwrap();
    };

    let ent_paths = ["this/that", "other", "yet/another/one"];
    let bundles = ent_paths
        .iter()
        .flat_map(|ent_path| create_bundles(store1, *ent_path))
        .collect_vec();

    for bundle in &bundles {
        insert(store1, bundle);
    }

    if let err @ Err(_) = store1.sanity_check() {
        store1.sort_indices_if_needed();
        eprintln!("{store1}");
        err.unwrap();
    }

    // Dump the first store into the second one.
    for bundle in store1.as_msg_bundles(MsgId::name()) {
        insert(store2, &bundle);
    }

    // Dump the second store into the third one.
    for bundle in store2.as_msg_bundles(MsgId::name()) {
        insert(store3, &bundle);
    }

    let store1_df = store1.to_dataframe();
    let store2_df = store2.to_dataframe();
    let store3_df = store3.to_dataframe();

    dbg!(DataStoreStats::from_store(store1));
    dbg!(DataStoreStats::from_store(store3));

    assert_eq!(
        store1_df, store2_df,
        "First & second stores differ:\n{store1_df}\n{store2_df}"
    );
    assert_eq!(
        store1_df, store3_df,
        "First & third stores differ:\n{store1_df}\n{store3_df}"
    );
}

// ---

pub fn init_logs() {
    static INIT: AtomicBool = AtomicBool::new(false);

    if INIT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        re_log::setup_native_logging();
    }
}
