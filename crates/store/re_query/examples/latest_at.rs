use std::sync::Arc;

use anyhow::Context;
use arrow2::array::PrimitiveArray as Arrow2PrimitiveArray;
use itertools::Itertools;

use re_chunk::{Chunk, RowId};
use re_chunk_store::{ChunkStore, ChunkStoreHandle, LatestAtQuery};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_log_types::{build_frame_nr, Timeline};
use re_types::Component as _;
use re_types_core::Archetype as _;

use re_query::{clamped_zip_1x2, LatestAtResults};

// ---

fn main() -> anyhow::Result<()> {
    let store = store()?;
    eprintln!("store:\n{store}");

    let entity_path = "points";
    let timeline = Timeline::new_sequence("frame_nr");
    let query = LatestAtQuery::latest(timeline);
    eprintln!("query:{query:?}");

    let caches = re_query::QueryCache::new(store.clone());

    // First, get the (potentially cached) results for this query.
    let results: LatestAtResults = caches.latest_at(
        &query,
        &entity_path.into(),
        MyPoints::all_components().iter(), // no generics!
    );

    // The results can be accessed either through the low-level Chunk APIs, or the higher-level helpers.

    // Example of accessing the data using the higher-level APIs.
    //
    // These APIs will log errors instead of returning them.
    {
        let points = results.component_batch::<MyPoint>().context("missing")?;
        let colors = results.component_batch::<MyColor>().unwrap_or_default();
        let labels = results.component_batch::<MyLabel>().unwrap_or_default();

        // Then apply your instance-level joining logic, if any:
        let color_default_fn = || MyColor(0xFF00FFFF);
        let label_default_fn = || MyLabel("N/A".to_owned());
        let results = clamped_zip_1x2(points, colors, color_default_fn, labels, label_default_fn)
            .collect_vec();

        eprintln!("results 1:\n{results:#?}");
    }

    // Example of accessing the data using the Chunk APIs.
    //
    // Because a latest-at query can only ever return a single row's worth of data for each
    // individual component, the chunks returned here will be so-called unit chunks, which are
    // guaranteed to only contain a single row.
    {
        // * `get_required` returns an error if the chunk is missing.
        // * `get` returns an option.
        let points = results.get_required(&MyPoint::name())?;
        let colors = results.get(&MyColor::name());
        let labels = results.get(&MyLabel::name());

        // You can always use the standard deserialization path:
        let points = points.component_batch::<MyPoint>().context("missing")??;
        let labels = labels
            .and_then(|unit| unit.component_batch::<MyLabel>()?.ok())
            .unwrap_or_default();

        // Or, if you want every last bit of performance you can get, you can manipulate the raw
        // data directly:
        let colors = colors
            .context("missing")?
            .component_batch_raw_arrow2(&MyColor::name())
            .context("invalid")?;
        let colors = colors
            .as_any()
            .downcast_ref::<Arrow2PrimitiveArray<u32>>()
            .context("invalid")?;
        let colors = colors
            .values()
            .as_slice()
            .iter()
            .map(|&color| MyColor(color));

        // And finally apply your instance-level joining logic, if any:
        let color_default_fn = || MyColor(0xFF00FFFF);
        let label_default_fn = || MyLabel("N/A".to_owned());
        let results = clamped_zip_1x2(points, colors, color_default_fn, labels, label_default_fn)
            .collect_vec();

        eprintln!("results 2:\n{results:#?}");
    }

    Ok(())
}

// ---

fn store() -> anyhow::Result<ChunkStoreHandle> {
    let store = ChunkStore::new_handle(
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
                    &[MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)]
                        as &dyn re_types_core::ComponentBatch, //
                    &[MyColor::from_rgb(255, 0, 0)],
                    &[MyLabel("a".into()), MyLabel("b".into())],
                ],
            )
            .build()?;

        store.write().insert_chunk(&Arc::new(chunk))?;
    }

    Ok(store)
}
