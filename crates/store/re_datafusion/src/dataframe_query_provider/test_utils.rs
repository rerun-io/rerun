//! Test-only chunk builders shared by the `segment_store` and `cpu_worker`
//! test modules (and, once it lands, the `pipeline_v2` driver's).

use re_dataframe::external::re_chunk::{Chunk, RowId};
use re_log_types::Timeline;
use re_log_types::example_components::{MyLabel, MyPoints};

/// Build a single-row temporal chunk on `timeline_name` at `time`
/// carrying one `MyLabel` component, so the chunk has non-zero
/// stored bytes and the store's `latest_at` machinery has a
/// component to find.
pub fn temporal_chunk(entity: &str, timeline_name: &'static str, time: i64) -> Chunk {
    let timepoint = [(Timeline::new_sequence(timeline_name), time)];
    let labels = &[MyLabel(format!("{entity}@{time}"))];
    Chunk::builder(entity)
        .with_component_batches(
            RowId::new(),
            timepoint,
            [(MyPoints::descriptor_labels(), labels as _)],
        )
        .build()
        .unwrap()
}
