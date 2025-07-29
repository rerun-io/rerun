//! Library providing utilities to load MCAP files with Rerun.

use anyhow::{Context as _, Result};
use mcap::Summary;
use re_chunk::{Chunk, EntityPath, RowId, TimePoint};
use re_types::archetypes::RecordingInfo;

pub mod cdr;
pub(crate) mod dds;
pub mod decode;
pub mod schema;
pub mod util;

/// Build the [`RecordingInfo`] chunk using the message statistics from a [`Summary`].
pub fn build_recording_properties_chunk(summary: &Summary) -> Result<Chunk> {
    let properties = summary
        .stats
        .as_ref()
        .map(|s| RecordingInfo::new().with_start_time(s.message_start_time as i64))
        .unwrap_or_default();

    debug_assert!(
        TimePoint::default().is_static(),
        "TimePoint::default() is not considered static!"
    );

    Chunk::builder(EntityPath::properties())
        .with_archetype(RowId::new(), TimePoint::default(), &properties)
        .build()
        .with_context(|| "Failed to build recording properties chunk MCAP file!")
}
