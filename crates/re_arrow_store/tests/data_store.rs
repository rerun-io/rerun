//! Straightforward high-level API tests.
//!
//! Testing & demonstrating expected usage of the datastore APIs, no funny stuff.

use arrow2::array::{Array, UInt64Array};
use polars_core::{prelude::*, series::Series};
use re_arrow_store::{DataStore, TimeQuery, TimelineQuery};
use re_log_types::{
    datagen::{build_frame_nr, build_instances, build_some_point2d, build_some_rects},
    field_types::{Instance, Point2D, Rect2D},
    msg_bundle::{Component as _, MsgBundle},
    ComponentName, MsgId, ObjPath as EntityPath, TimeType, Timeline,
};

// ---

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
fn latest_at() {
    let mut store = DataStore::new(Instance::name(), Default::default());

    let ent_path = EntityPath::from("this/that");

    let (instances1, rects1) = (build_instances(3), build_some_rects(3));
    let bundle1 = test_bundle!(ent_path @ [build_frame_nr(1)] => [instances1.clone(), rects1]);
    store.insert(&bundle1).unwrap();

    let points2 = build_some_point2d(3);
    let bundle2 = test_bundle!(ent_path @ [build_frame_nr(2)] => [instances1, points2]);
    store.insert(&bundle2).unwrap();

    let points3 = build_some_point2d(10);
    let bundle3 = test_bundle!(ent_path @ [build_frame_nr(3)] => [points3]);
    store.insert(&bundle3).unwrap();

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
    }

    assert_scenario(&store, &ent_path, 0, &[]);
    assert_scenario(&store, &ent_path, 1, &[(Rect2D::name(), &bundle1)]);
    assert_scenario(
        &store,
        &ent_path,
        2,
        &[(Rect2D::name(), &bundle1), (Point2D::name(), &bundle2)],
    );
    // TODO: that is where implicit instances should be shown to work!
    // assert_scenario(
    //     &store,
    //     &ent_path,
    //     3,
    //     &[(Rect2D::name(), &bundle1), (Point2D::name(), &bundle3)],
    // );
}

fn assert_scenario(
    store: &DataStore,
    ent_path: &EntityPath,
    frame_nr: i64,
    bundles: &[(ComponentName, &MsgBundle)],
) {
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let components_all = &[Rect2D::name(), Point2D::name()];

    let df = joined_query(
        store,
        &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame_nr)),
        ent_path,
        components_all,
    );

    let df_expected = new_expected_df(bundles);
    assert_eq!(df_expected, df);
}

// TODO(cmc): range API tests!

// --- Helpers ---

// Queries a bunch of components and their clustering keys, joins everything, and returns the
// resulting `DataFrame`.
fn joined_query(
    store: &DataStore,
    timeline_query: &TimelineQuery,
    ent_path: &EntityPath,
    primaries: &[ComponentName],
) -> DataFrame {
    let dfs = primaries
        .iter()
        .map(|primary| query(store, timeline_query, ent_path, *primary))
        .filter(|df| !df.is_empty());

    dfs.reduce(|acc, df| {
        acc.left_join(
            dbg!(&df),
            [Instance::name().as_str()],
            [Instance::name().as_str()],
        )
        .unwrap()
    })
    .unwrap_or_default()
}

/// Query a single component and its clustering key, returns a `DataFrame`.
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

// TODO: doc
fn new_expected_df(bundles: &[(ComponentName, &MsgBundle)]) -> DataFrame {
    bundles
        .iter()
        .map(|(component, bundle)| {
            let df = if bundle.components.len() == 1 {
                DataFrame::new(vec![
                    Series::try_from((
                        Instance::name().as_str(),
                        UInt64Array::from_vec(
                            (0..bundle.components[0].value.len() as u64).collect(),
                        )
                        .to_boxed(),
                    ))
                    .unwrap(),
                    Series::try_from((component.as_str(), bundle.components[0].value.to_boxed()))
                        .unwrap(),
                ])
                .unwrap()
            } else {
                DataFrame::new(vec![
                    Series::try_from((
                        Instance::name().as_str(),
                        bundle.components[0].value.to_boxed(),
                    ))
                    .unwrap(),
                    Series::try_from((component.as_str(), bundle.components[1].value.to_boxed()))
                        .unwrap(),
                ])
                .unwrap()
            };
            df.explode(df.get_column_names()).unwrap()
        })
        .reduce(|acc, df| {
            acc.left_join(
                &df,
                [Instance::name().as_str()],
                [Instance::name().as_str()],
            )
            .unwrap()
        })
        .unwrap_or_default()
}
