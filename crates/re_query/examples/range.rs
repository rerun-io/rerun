use std::sync::Arc;

use itertools::Itertools;
use re_data_store2::external::re_chunk::Chunk;
use re_data_store2::{DataStore2, RangeQuery};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_log_types::{build_frame_nr, ResolvedTimeRange, RowId, TimeType, Timeline};
use re_types::ComponentBatch;
use re_types_core::{Archetype as _, Loggable as _};

use re_query::{
    clamped_zip_1x2, range_zip_1x2, PromiseResolver, PromiseResult, RangeComponentResults,
    RangeResults,
};

// ---

fn main() -> anyhow::Result<()> {
    let store = store()?;
    eprintln!("store:\n{store}");

    let resolver = PromiseResolver::default();

    let entity_path = "points";
    let timeline = Timeline::new("frame_nr", TimeType::Sequence);
    let query = RangeQuery::new(timeline, ResolvedTimeRange::EVERYTHING);
    eprintln!("query:{query:?}");

    let caches = re_query::Caches::new(&store);

    // First, get the raw results for this query.
    //
    // They might or might not already be cached. We won't know for sure until we try to access
    // each individual component's data below.
    let results: RangeResults = caches.range(
        &store,
        &query,
        &entity_path.into(),
        MyPoints::all_components().iter().copied(), // no generics!
    );

    // Then, grab the results for each individual components.
    // * `get_required` returns an error if the component batch is missing
    // * `get_or_empty` returns an empty set of results if the component if missing
    // * `get` returns an option
    //
    // At this point we still don't know whether they are cached or not. That's the next step.
    let all_points: &RangeComponentResults = results.get_required(MyPoint::name())?;
    let all_colors: &RangeComponentResults = results.get_or_empty(MyColor::name());
    let all_labels: &RangeComponentResults = results.get_or_empty(MyLabel::name());

    // Then comes the time to resolve/convert and deserialize the data.
    // These steps have to be done together for efficiency reasons.
    //
    // That's when caching comes into play.
    // If the data has already been accessed in the past, then this will just grab the
    // pre-deserialized, pre-resolved/pre-converted result from the cache.
    // Otherwise, this will trigger a deserialization and cache the result for next time.
    let all_points = all_points.to_dense::<MyPoint>(&resolver);
    let all_colors = all_colors.to_dense::<MyColor>(&resolver);
    let all_labels = all_labels.to_dense::<MyLabel>(&resolver);

    // The cache might not have been able to resolve and deserialize the entire dataset across all
    // available timestamps.
    //
    // We can use the following APIs to check the status of the front and back sides of the data range.
    //
    // E.g. it is possible that the front-side of the range is still waiting for pending data while
    // the back-side has been fully loaded.
    assert!(matches!(
        all_points.status(),
        (PromiseResult::Ready(()), PromiseResult::Ready(()))
    ));

    // Zip the results together using a stateful time-based join.
    let all_frames = range_zip_1x2(
        all_points.range_indexed(),
        all_colors.range_indexed(),
        all_labels.range_indexed(),
    );

    // Then comes the time to resolve/convert and deserialize the data, _for each timestamp_.
    // These steps have to be done together for efficiency reasons.
    //
    // Both the resolution and deserialization steps might fail, which is why this returns a `Result<Result<T>>`.
    // Use `PromiseResult::flatten` to simplify it down to a single result.
    eprintln!("results:");
    for ((data_time, row_id), points, colors, labels) in all_frames {
        let colors = colors.unwrap_or(&[]);
        let color_default_fn = || {
            static DEFAULT: MyColor = MyColor(0xFF00FFFF);
            &DEFAULT
        };

        let labels = labels.unwrap_or(&[]).iter().cloned().map(Some);
        let label_default_fn = || None;

        // With the data now fully resolved/converted and deserialized, the joining logic can be
        // applied.
        //
        // In most cases this will be either a clamped zip, or no joining at all.

        let results = clamped_zip_1x2(points, colors, color_default_fn, labels, label_default_fn)
            .collect_vec();
        eprintln!("{data_time:?} @ {row_id}:\n    {results:?}");
    }

    Ok(())
}

// ---

fn store() -> anyhow::Result<DataStore2> {
    let mut store = DataStore2::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );

    let entity_path = "points";

    {
        let timepoint = [build_frame_nr(123)];

        let chunk = Chunk::builder(entity_path.into())
            .with_component_batches(
                RowId::new(),
                timepoint,
                [
                    &[MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)] as &dyn ComponentBatch, //
                    &[MyColor::from_rgb(255, 0, 0)],
                    &[MyLabel("a".into()), MyLabel("b".into())],
                ],
            )
            .build()?;

        store.insert_chunk(&Arc::new(chunk))?;
    }

    {
        let timepoint = [build_frame_nr(423)];

        let chunk = Chunk::builder(entity_path.into())
            .with_component_batches(
                RowId::new(),
                timepoint,
                [
                    &[
                        MyPoint::new(10.0, 20.0),
                        MyPoint::new(30.0, 40.0),
                        MyPoint::new(50.0, 60.0),
                    ] as &dyn ComponentBatch, //
                    &[MyColor::from_rgb(255, 0, 0), MyColor::from_rgb(0, 0, 255)],
                ],
            )
            .build()?;

        store.insert_chunk(&Arc::new(chunk))?;
    }

    Ok(store)
}
