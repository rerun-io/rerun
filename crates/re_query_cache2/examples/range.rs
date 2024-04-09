use itertools::Itertools;
use re_data_store::{DataStore, RangeQuery};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_log_types::{build_frame_nr, DataRow, RowId, TimeRange, TimeType, Timeline};
use re_types_core::{Archetype as _, Loggable as _};

use re_query_cache2::{
    clamped_zip_1x2, range_zip_1x2, CachedRangeComponentResults, CachedRangeResults,
    PromiseResolver, PromiseResult,
};

// ---

fn main() -> anyhow::Result<()> {
    let store = store()?;
    eprintln!("store:\n{}", store.to_data_table()?);

    let resolver = PromiseResolver::default();

    let entity_path = "points";
    let timeline = Timeline::new("frame_nr", TimeType::Sequence);
    let query = RangeQuery::new(timeline, TimeRange::EVERYTHING);
    eprintln!("query:{query:?}");

    let caches = re_query_cache2::Caches::new(&store);

    // First, get the raw results for this query.
    //
    // They might or might not already be cached. We won't know for sure until we try to access
    // each individual component's data below.
    let results: CachedRangeResults = caches.range(
        &store,
        &query,
        &entity_path.into(),
        MyPoints::all_components().iter().cloned(), // no generics!
    );

    // Then, grab the results for each individual components.
    // * `get_required` returns an error if the component batch is missing
    // * `get_or_empty` returns an empty set of results if the component if missing
    // * `get` returns an option
    //
    // At this point we still don't know whether they are cached or not. That's the next step.
    let all_points: &CachedRangeComponentResults = results.get_required(MyPoint::name())?;
    let all_colors: &CachedRangeComponentResults = results.get_or_empty(MyColor::name());
    let all_labels: &CachedRangeComponentResults = results.get_or_empty(MyLabel::name());

    // Then comes the time to resolve/convert and deserialize the data.
    // These steps have to be done together for efficiency reasons.
    //
    // A choice now has to be made regarding the nullability of the _component batch's instances_.
    // Our IDL doesn't support nullable instances at the moment -- so for the foreseeable future you probably
    // shouldn't be using anything but `iter_dense`.
    //
    // This is the step at which caching comes into play.
    //
    // If the data has already been accessed with the same nullability characteristics in the
    // past, then this will just grab the pre-deserialized, pre-resolved/pre-converted result from
    // the cache.
    //
    // Otherwise, this will trigger a deserialization and cache the result for next time.
    let all_points = all_points.to_dense::<MyPoint>(&resolver);
    let all_colors = all_colors.to_sparse::<MyColor>(&resolver);
    let all_labels = all_labels.to_sparse::<MyLabel>(&resolver);

    // The cache might not have been able to resolve and deserialize the entire dataset across all
    // available timestamps.
    //
    // We can use the following APIs to check the status of the front back sides of the data range.
    //
    // E.g. it is possible that the front-side of the range is still waiting for pending data while
    // the back-side has been fully loaded.
    assert!(matches!(
        all_points.status(query.range()),
        (PromiseResult::Ready(()), PromiseResult::Ready(()))
    ));

    // Zip the results together using a stateful time-based join.
    let all_frames = range_zip_1x2(
        all_points.range_indexed(query.range()),
        all_colors.range_indexed(query.range()),
        all_labels.range_indexed(query.range()),
    );

    // Then comes the time to resolve/convert and deserialize the data, _for each timestamp_.
    // These steps have to be done together for efficiency reasons.
    //
    // Both the resolution and deserialization steps might fail, which is why this returns a `Result<Result<T>>`.
    // Use `PromiseResult::flatten` to simplify it down to a single result.
    //
    // A choice now has to be made regarding the nullability of the _component batch's instances_.
    // Our IDL doesn't support nullable instances at the moment -- so for the foreseeable future you probably
    // shouldn't be using anything but `iter_dense`.
    eprintln!("results:");
    for ((data_time, row_id), points, colors, labels) in all_frames {
        let colors = colors.unwrap_or(&[]);
        let color_default_fn = || {
            static DEFAULT: Option<MyColor> = Some(MyColor(0xFF00FFFF));
            &DEFAULT
        };

        let labels = labels.unwrap_or(&[]);
        let label_default_fn = || &None;

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

fn store() -> anyhow::Result<DataStore> {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        re_types::components::InstanceKey::name(),
        Default::default(),
    );

    let entity_path = "points";

    {
        let timepoint = [build_frame_nr(123)];

        let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, points)?;
        store.insert_row(&row)?;

        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 1, colors)?;
        store.insert_row(&row)?;

        let labels = vec![MyLabel("a".into()), MyLabel("b".into())];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, labels)?;
        store.insert_row(&row)?;
    }

    {
        let timepoint = [build_frame_nr(456)];

        let colors = vec![MyColor::from_rgb(255, 0, 0), MyColor::from_rgb(0, 0, 255)];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 1, colors)?;
        store.insert_row(&row)?;

        let points = vec![
            MyPoint::new(10.0, 20.0),
            MyPoint::new(30.0, 40.0),
            MyPoint::new(50.0, 60.0),
        ];
        let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, points)?;
        store.insert_row(&row)?;
    }

    Ok(store)
}
