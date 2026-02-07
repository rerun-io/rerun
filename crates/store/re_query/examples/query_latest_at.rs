use std::sync::Arc;

use anyhow::Context as _;
use arrow::array::UInt32Array as ArrowUInt32Array;
use itertools::Itertools as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{Chunk, RowId, TimelineName};
use re_chunk_store::{ChunkStore, ChunkStoreHandle, LatestAtQuery};
use re_log_types::build_frame_nr;
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_query::{LatestAtResults, clamped_zip_1x2};
use re_types_core::Archetype as _;

// ---

fn main() -> anyhow::Result<()> {
    let store = store()?;
    eprintln!("store:\n{store}");

    let entity_path = "points";
    let timeline = TimelineName::new("frame_nr");
    let query = LatestAtQuery::latest(timeline);
    eprintln!("query:{query:?}");

    let caches = re_query::QueryCache::new(store.clone());

    // First, get the (potentially cached) results for this query.
    let results: LatestAtResults = caches.latest_at(
        &query,
        &entity_path.into(),
        MyPoints::all_component_identifiers(),
    );

    // The results can be accessed either through the low-level Chunk APIs, or the higher-level helpers.

    // Example of accessing the data using the higher-level APIs.
    //
    // These APIs will log errors instead of returning them.
    {
        let points = results
            .component_batch::<MyPoint>(MyPoints::descriptor_points().component)
            .context("missing")?;
        let colors = results
            .component_batch::<MyColor>(MyPoints::descriptor_colors().component)
            .unwrap_or_default();
        let labels = results
            .component_batch::<MyLabel>(MyPoints::descriptor_labels().component)
            .unwrap_or_default();

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
        let points = results.get_required(MyPoints::descriptor_points().component)?;
        let colors = results.get(MyPoints::descriptor_colors().component);
        let labels = results.get(MyPoints::descriptor_labels().component);

        // You can always use the standard deserialization path:
        let points = points
            .component_batch::<MyPoint>(MyPoints::descriptor_points().component)
            .context("missing")??;
        let labels = labels
            .and_then(|unit| {
                unit.component_batch::<MyLabel>(MyPoints::descriptor_labels().component)?
                    .ok()
            })
            .unwrap_or_default();

        // Or, if you want every last bit of performance you can get, you can manipulate the raw
        // data directly:
        let colors = colors
            .context("missing")?
            .component_batch_raw(MyPoints::descriptor_colors().component)
            .context("invalid")?;
        let colors = colors
            .downcast_array_ref::<ArrowUInt32Array>()
            .context("invalid")?;
        let colors = colors.values().iter().map(|&color| MyColor(color));

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
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        Default::default(),
    );

    let entity_path = "points";

    {
        let timepoint = [build_frame_nr(123)];

        let chunk = Chunk::builder(entity_path)
            .with_archetype(
                RowId::new(),
                timepoint,
                &MyPoints::new([MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)])
                    .with_colors([MyColor::from_rgb(255, 0, 0)])
                    .with_labels([MyLabel("a".into()), MyLabel("b".into())]),
            )
            .build()?;

        store.write().insert_chunk(&Arc::new(chunk))?;
    }

    Ok(store)
}
