//! Straightforward high-level API tests.
//!
//! Testing & demonstrating expected usage of the datastore APIs, no funny stuff.

use arrow2::array::Array;

use polars_core::{prelude::*, series::Series};
use re_arrow_store::{DataStore, TimeQuery, TimelineQuery};
use re_log_types::{
    datagen::{build_frame_nr, build_instances, build_some_rects},
    field_types::{Instance, Point2D, Rect2D},
    msg_bundle::Component as _,
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

    let bundle1 = test_bundle!(ent_path @ [build_frame_nr(1)] => [
        build_instances(3),
        build_some_rects(3),
    ]);
    store.insert(&bundle1).unwrap();

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
    }

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let components_all = &[Rect2D::name(), Point2D::name()];

    let df = joined_query(
        &store,
        &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(1)),
        &ent_path,
        components_all,
    );
    let df_expected = new_expected_df(&[(
        Rect2D::name(),
        (&*bundle1.components[0].value, &*bundle1.components[1].value),
    )]);
    dbg!(df);
    dbg!(df_expected);
}

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
            &df,
            [Instance::name().as_str()],
            [Instance::name().as_str()],
        )
        .unwrap()
    })
    .unwrap()
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

/// `components` is `&[(name, (instance_data, component_data))]`
#[allow(clippy::type_complexity)]
fn new_expected_df(components: &[(ComponentName, (&dyn Array, &dyn Array))]) -> DataFrame {
    let df = components
        .iter()
        .map(|(component, (instance_data, component_data))| {
            DataFrame::new(vec![
                Series::try_from((Instance::name().as_str(), instance_data.to_boxed())).unwrap(),
                Series::try_from((component.as_str(), component_data.to_boxed())).unwrap(),
            ])
            .unwrap()
        })
        .reduce(|acc, df| {
            acc.left_join(
                &df,
                [Instance::name().as_str()],
                [Instance::name().as_str()],
            )
            .unwrap()
        })
        .unwrap();
    df.explode(df.get_column_names()).unwrap()
}
