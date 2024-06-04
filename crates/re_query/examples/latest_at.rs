use std::sync::Arc;

use itertools::Itertools;

use re_data_store2::external::re_chunk::Chunk;
use re_data_store2::{DataStore2, LatestAtQuery};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_log_types::{build_frame_nr, RowId, TimeType, Timeline};
use re_types::ComponentBatch;
use re_types_core::{Archetype as _, Loggable as _};

use re_query::{
    clamped_zip_1x2, LatestAtComponentResults, LatestAtResults, PromiseResolver, PromiseResult,
};

// ---

fn main() -> anyhow::Result<()> {
    let store = store()?;
    eprintln!("store:\n{store}");

    let resolver = PromiseResolver::default();

    let entity_path = "points";
    let timeline = Timeline::new("frame_nr", TimeType::Sequence);
    let query = LatestAtQuery::latest(timeline);
    eprintln!("query:{query:?}");

    let caches = re_query::Caches::new(&store);

    // First, get the results for this query.
    //
    // They might or might not already be cached. We won't know for sure until we try to access
    // each individual component's data below.
    let results: LatestAtResults = caches.latest_at(
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
    let points: &LatestAtComponentResults = results.get_required(MyPoint::name())?;
    let colors: &LatestAtComponentResults = results.get_or_empty(MyColor::name());
    let labels: &LatestAtComponentResults = results.get_or_empty(MyLabel::name());

    // Then comes the time to resolve/convert and deserialize the data.
    // These steps have to be done together for efficiency reasons.
    //
    // Both the resolution and deserialization steps might fail, which is why this returns a `Result<Result<T>>`.
    // Use `PromiseResult::flatten` to simplify it down to a single result.
    //
    // This is the step at which caching comes into play.
    // If the data has already been accessed in the past, then this will just grab the pre-deserialized,
    // pre-resolved/pre-converted result from the cache.
    // Otherwise, this will trigger a deserialization and cache the result for next time.

    let points = match points.iter_dense::<MyPoint>(&resolver).flatten() {
        PromiseResult::Pending => {
            // Handle the fact that the data isn't ready appropriately.
            return Ok(());
        }
        PromiseResult::Ready(data) => data,
        PromiseResult::Error(err) => return Err(err.into()),
    };

    let colors = match colors.iter_dense::<MyColor>(&resolver).flatten() {
        PromiseResult::Pending => {
            // Handle the fact that the data isn't ready appropriately.
            return Ok(());
        }
        PromiseResult::Ready(data) => data,
        PromiseResult::Error(err) => return Err(err.into()),
    };

    let labels = match labels.iter_dense::<MyLabel>(&resolver).flatten() {
        PromiseResult::Pending => {
            // Handle the fact that the data isn't ready appropriately.
            return Ok(());
        }
        PromiseResult::Ready(data) => data.map(Some),
        PromiseResult::Error(err) => return Err(err.into()),
    };

    // With the data now fully resolved/converted and deserialized, the joining logic can be
    // applied.
    //
    // In most cases this will be either a clamped zip, or no joining at all.

    let color_default_fn = || {
        static DEFAULT: MyColor = MyColor(0xFF00FFFF);
        &DEFAULT
    };
    let label_default_fn = || None;

    let results =
        clamped_zip_1x2(points, colors, color_default_fn, labels, label_default_fn).collect_vec();

    eprintln!("results:\n{results:#?}");

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

    Ok(store)
}
