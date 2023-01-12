use std::sync::atomic::{AtomicBool, Ordering};

use arrow2::array::ListArray;
use rand::{distributions::Uniform, Rng};
use re_arrow_store::{
    test_bundle, DataStore, DataStoreConfig, DataStoreStats, GarbageCollectionTarget,
};
use re_log_types::{
    datagen::{build_frame_nr, build_some_rects},
    external::arrow2_convert::deserialize::{arrow_array_deserialize_iterator, ArrowDeserialize},
    field_types::Instance,
    msg_bundle::Component,
    MsgId, ObjPath as EntityPath,
};

// ---

#[test]
fn gc() {
    init_logs();

    for config in re_arrow_store::test_util::all_configs() {
        let mut store = DataStore::new(Instance::name(), config.clone());
        gc_impl(&mut store);
    }
}
fn gc_impl(store: &mut DataStore) {
    let mut rng = rand::thread_rng();

    for _ in 0..2 {
        let nb_ents = 10;
        for i in 0..nb_ents {
            let ent_path = EntityPath::from(format!("this/that/{i}"));

            let nb_frames = rng.gen_range(0..=100);
            let frames = (0..nb_frames).filter(|_| rand::thread_rng().gen());
            for frame_nr in frames {
                let nb_instances = rng.gen_range(0..=1_000);
                let bundle = test_bundle!(ent_path @ [build_frame_nr(frame_nr.into())] => [
                    build_some_rects(nb_instances),
                ]);
                store.insert(&bundle).unwrap();
            }
        }

        if let err @ Err(_) = store.sanity_check() {
            store.sort_indices_if_needed();
            eprintln!("{store}");
            err.unwrap();
        }

        let msg_id_chunks = store
            .gc(
                MsgId::name(),
                GarbageCollectionTarget::DropAtLeastPercentage(1.0 / 3.0),
            )
            .unwrap();

        let msg_ids = msg_id_chunks
            .iter()
            .flat_map(|chunk| arrow_array_deserialize_iterator::<Option<MsgId>>(&**chunk).unwrap())
            .map(Option::unwrap) // MsgId is always present
            .collect::<ahash::HashSet<_>>();

        for msg_id in &msg_ids {
            assert!(store.get_msg_metadata(msg_id).is_some());
        }

        store.clear_msg_metadata(&msg_ids);

        for msg_id in &msg_ids {
            assert!(store.get_msg_metadata(msg_id).is_none());
        }
    }
}

#[test]
fn gc_correct() {
    init_logs();

    let mut store = DataStore::new(
        Instance::name(),
        DataStoreConfig {
            component_bucket_size_bytes: 0,
            component_bucket_nb_rows: 0,
            ..Default::default()
        },
    );

    let mut rng = rand::thread_rng();

    let nb_ents = 10;
    for i in 0..nb_ents {
        let ent_path = EntityPath::from(format!("this/that/{i}"));

        let nb_frames = rng.gen_range(0..=100);
        let frames = (0..nb_frames).filter(|_| rand::thread_rng().gen());
        for frame_nr in frames {
            let nb_instances = rng.gen_range(0..=1_000);
            let bundle = test_bundle!(ent_path @ [build_frame_nr(frame_nr.into())] => [
                build_some_rects(nb_instances),
            ]);
            store.insert(&bundle).unwrap();
        }
    }

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices_if_needed();
        eprintln!("{store}");
        err.unwrap();
    }
    _ = store.to_dataframe(); // nice way to check everything is readable

    dbg!(DataStoreStats::from_store(&store).total_temporal_component_rows);

    let msg_id_chunks = store
        .gc(
            MsgId::name(),
            GarbageCollectionTarget::DropAtLeastPercentage(1.0),
        )
        .unwrap();

    let msg_ids = msg_id_chunks
        .iter()
        .flat_map(|chunk| arrow_array_deserialize_iterator::<Option<MsgId>>(&**chunk).unwrap())
        .map(Option::unwrap) // MsgId is always present
        .collect::<ahash::HashSet<_>>();

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices_if_needed();
        eprintln!("{store}");
        err.unwrap();
    }
    _ = store.to_dataframe(); // nice way to check everything is readable
    for msg_id in &msg_ids {
        assert!(store.get_msg_metadata(msg_id).is_some());
    }

    store.clear_msg_metadata(&msg_ids);

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices_if_needed();
        eprintln!("{store}");
        err.unwrap();
    }
    _ = store.to_dataframe(); // nice way to check everything is readable
    for msg_id in &msg_ids {
        assert!(store.get_msg_metadata(msg_id).is_none());
    }

    dbg!(DataStoreStats::from_store(&store).total_temporal_component_rows);

    let msg_id_chunks = store
        .gc(
            MsgId::name(),
            GarbageCollectionTarget::DropAtLeastPercentage(1.0),
        )
        .unwrap();

    let msg_ids = msg_id_chunks
        .iter()
        .flat_map(|chunk| arrow_array_deserialize_iterator::<Option<MsgId>>(&**chunk).unwrap())
        .map(Option::unwrap) // MsgId is always present
        .collect::<ahash::HashSet<_>>();
    assert!(msg_ids.is_empty());

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices_if_needed();
        eprintln!("{store}");
        err.unwrap();
    }
    _ = store.to_dataframe(); // nice way to check everything is readable

    dbg!(DataStoreStats::from_store(&store).total_temporal_component_rows);

    // eprintln!("{store}");
    // dbg!(store.to_dataframe());
}

// ---

pub fn init_logs() {
    static INIT: AtomicBool = AtomicBool::new(false);

    if INIT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        re_log::set_default_rust_log_env();
        tracing_subscriber::fmt::init(); // log to stdout
    }
}
