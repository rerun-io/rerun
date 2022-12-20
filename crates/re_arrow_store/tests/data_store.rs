//! Straightforward high-level API tests.
//!
//! Testing & demonstrating expected usage of the datastore APIs, no funny stuff.

use std::sync::atomic::{AtomicBool, Ordering};

use arrow2::array::{Array, ListArray, UInt32Array};
use polars_core::{prelude::*, series::Series};
use re_arrow_store::{test_bundle, DataStore, TimeQuery, TimelineQuery};
use re_log_types::{
    datagen::{build_frame_nr, build_instances, build_some_point2d, build_some_rects},
    field_types::{Instance, Point2D, Rect2D},
    msg_bundle::{wrap_in_listarray, Component as _, MsgBundle},
    ComponentName, MsgId, ObjPath as EntityPath, TimeType, Timeline,
};

// --- LatestAt ---

#[test]
fn latest_at_() {
    init_logs();

    for config in re_arrow_store::test_util::all_configs() {
        let mut store = DataStore::new(Instance::name(), config.clone());
        latest_at_impl(&mut store);
    }
}
fn latest_at_impl(store: &mut DataStore) {
    init_logs();

    let ent_path = EntityPath::from("this/that");

    let frame0 = 0;
    let frame1 = 1;
    let frame2 = 2;
    let frame3 = 3;
    let frame4 = 4;

    let (instances1, rects1) = (build_instances(3), build_some_rects(3));
    let bundle1 = test_bundle!(ent_path @ [build_frame_nr(1)] => [instances1.clone(), rects1]);
    store.insert(&bundle1).unwrap();

    let points2 = build_some_point2d(3);
    let bundle2 = test_bundle!(ent_path @ [build_frame_nr(2)] => [instances1, points2]);
    store.insert(&bundle2).unwrap();

    let points3 = build_some_point2d(10);
    let bundle3 = test_bundle!(ent_path @ [build_frame_nr(3)] => [points3]);
    store.insert(&bundle3).unwrap();

    let rects4 = build_some_rects(5);
    let bundle4 = test_bundle!(ent_path @ [build_frame_nr(4)] => [rects4]);
    store.insert(&bundle4).unwrap();

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
    }

    assert_joint_query_at(store, &ent_path, frame0, &[]);
    assert_joint_query_at(store, &ent_path, frame1, &[(Rect2D::name(), &bundle1)]);
    assert_joint_query_at(
        store,
        &ent_path,
        frame2,
        &[(Rect2D::name(), &bundle1), (Point2D::name(), &bundle2)],
    );
    assert_joint_query_at(
        store,
        &ent_path,
        frame3,
        &[(Rect2D::name(), &bundle1), (Point2D::name(), &bundle3)],
    );
    assert_joint_query_at(
        store,
        &ent_path,
        frame4,
        &[(Rect2D::name(), &bundle4), (Point2D::name(), &bundle3)],
    );
}

/// Runs a joint query over all components at the given `frame_nr`, and asserts that the result
/// matches a joint `DataFrame` built ouf of the specified raw `bundles`.
fn assert_joint_query_at(
    store: &mut DataStore,
    ent_path: &EntityPath,
    frame_nr: i64,
    bundles: &[(ComponentName, &MsgBundle)],
) {
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let components_all = &[Rect2D::name(), Point2D::name()];

    let df = joint_query(
        store,
        &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame_nr)),
        ent_path,
        components_all,
    );

    let df_expected = joint_df(bundles);

    store.sort_indices();
    assert_eq!(df_expected, df, "{store}");
}

// --- Helpers ---

// Queries a bunch of components and their clustering keys, joins everything together, and returns
// the resulting `DataFrame`.
// TODO: doc
fn joint_query(
    store: &DataStore,
    timeline_query: &TimelineQuery,
    ent_path: &EntityPath,
    primaries: &[ComponentName],
) -> DataFrame {
    let dfs = primaries
        .iter()
        .map(|primary| query(store, timeline_query, ent_path, *primary))
        .filter(|df| !df.is_empty());

    let df = dfs
        .reduce(|acc, df| {
            acc.outer_join(
                &df,
                [Instance::name().as_str()],
                [Instance::name().as_str()],
            )
            .unwrap()
        })
        .unwrap_or_default();

    df.sort([Instance::name().as_str()], false).unwrap_or(df)
}

/// Query a single component and its clustering key, returns a `DataFrame`.
// TODO: doc
fn query(
    store: &DataStore,
    timeline_query: &TimelineQuery,
    ent_path: &EntityPath,
    primary: ComponentName,
) -> DataFrame {
    let components = &[Instance::name(), primary];
    let row_indices = store
        .query(timeline_query, ent_path, primary, components)
        .unwrap_or([None; 2]);
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

/// Builds a joint `DataFrame` directly out of raw bundles, mimicking the behaviour of a joint
/// query on the datastore.
// TODO: doc
fn joint_df(bundles: &[(ComponentName, &MsgBundle)]) -> DataFrame {
    let df = bundles
        .iter()
        .map(|(component, bundle)| {
            let instances = if bundle.components.len() == 1 {
                let len = bundle.components[0]
                    .value
                    .as_any()
                    .downcast_ref::<ListArray<i32>>()
                    .unwrap()
                    .value(0)
                    .len();
                Series::try_from((
                    Instance::name().as_str(),
                    wrap_in_listarray(UInt32Array::from_vec((0..len as u32).collect()).to_boxed())
                        .to_boxed(),
                ))
                .unwrap()
            } else {
                Series::try_from((
                    Instance::name().as_str(),
                    bundle.components[0].value.to_boxed(),
                ))
                .unwrap()
            };

            let df = DataFrame::new(vec![
                instances,
                Series::try_from((
                    component.as_str(),
                    bundle.components.last().unwrap().value.to_boxed(),
                ))
                .unwrap(),
            ])
            .unwrap();

            df.explode(df.get_column_names()).unwrap()
        })
        .reduce(|acc, df| {
            acc.outer_join(
                &df,
                [Instance::name().as_str()],
                [Instance::name().as_str()],
            )
            .unwrap()
        })
        .unwrap_or_default();

    df.sort([Instance::name().as_str()], false).unwrap_or(df)
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
