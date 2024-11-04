use std::sync::Arc;

use itertools::{izip, Itertools};
use re_chunk::{Chunk, RowId};
use re_chunk_store::{ChunkStore, ChunkStoreHandle, RangeQuery};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_log_types::{build_frame_nr, ResolvedTimeRange, TimeType, Timeline};
use re_types::ComponentBatch;
use re_types_core::{Archetype as _, Loggable as _};

use re_query::{clamped_zip_1x2, range_zip_1x2, RangeResults};

// ---

fn main() -> anyhow::Result<()> {
    let store = store()?;
    eprintln!("store:\n{store}");

    let entity_path = "points";
    let timeline = Timeline::new("frame_nr", TimeType::Sequence);
    let query = RangeQuery::new(timeline, ResolvedTimeRange::EVERYTHING);
    eprintln!("query:{query:?}");

    let caches = re_query::QueryCache::new(store.clone());

    // First, get the (potentially cached) results for this query.
    let results: RangeResults = caches.range(
        &query,
        &entity_path.into(),
        MyPoints::all_components().iter().copied(), // no generics!
    );

    // * `get_required` returns an error if the chunk is missing.
    // * `get` returns an option.
    let all_points_chunks = results.get_required(&MyPoint::name())?;
    let all_colors_chunks = results.get(&MyColor::name());
    let all_labels_chunks = results.get(&MyLabel::name());

    // You can always use the standard deserialization path.
    //
    // The underlying operator is optimized to only pay the cost of downcasting and deserialization
    // once for the whole column, and will then return references into that data.
    // This is why you have to process the data in two-steps: the iterator needs to have somewhere
    // to reference to.
    let all_points_indexed = all_points_chunks.iter().flat_map(|chunk| {
        izip!(
            chunk.iter_component_indices(&query.timeline(), &MyPoint::name()),
            chunk.iter_component::<MyPoint>()
        )
    });
    let all_labels_indexed = all_labels_chunks
        .unwrap_or_default()
        .iter()
        .flat_map(|chunk| {
            izip!(
                chunk.iter_component_indices(&query.timeline(), &MyLabel::name()),
                chunk.iter_component::<MyLabel>()
            )
        });

    // Or, if you want every last bit of performance you can get, you can manipulate the raw
    // data directly:
    let all_colors_indexed = all_colors_chunks
        .unwrap_or_default()
        .iter()
        .flat_map(|chunk| {
            izip!(
                chunk.iter_component_indices(&query.timeline(), &MyColor::name()),
                chunk.iter_primitive::<u32>(&MyColor::name()),
            )
        });

    // Zip the results together using a stateful time-based join.
    let all_frames = range_zip_1x2(all_points_indexed, all_colors_indexed, all_labels_indexed);

    // And finally inspect our final results:
    {
        let color_default_fn = || Some(MyColor(0xFF00FFFF));
        let label_default_fn = || None;

        eprintln!("results:");
        for ((data_time, row_id), points, colors, labels) in all_frames {
            let points = points.as_slice();
            let colors = colors.unwrap_or_default().iter().map(|c| Some(MyColor(*c)));
            let labels = labels.unwrap_or_default();
            let labels = labels.as_slice().iter().cloned().map(Some);

            // Apply your instance-level joining logic, if any:
            let results =
                clamped_zip_1x2(points, colors, color_default_fn, labels, label_default_fn)
                    .collect_vec();
            eprintln!("{data_time:?} @ {row_id}:\n    {results:?}");
        }
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
                    &[MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)] as &dyn ComponentBatch, //
                    &[MyColor::from_rgb(255, 0, 0)],
                    &[MyLabel("a".into()), MyLabel("b".into())],
                ],
            )
            .build()?;

        store.write().insert_chunk(&Arc::new(chunk))?;
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

        store.write().insert_chunk(&Arc::new(chunk))?;
    }

    Ok(store)
}
