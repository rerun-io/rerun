use re_chunk::{Chunk, EntityPath, RowId, TimePoint};
use re_sdk_types::archetypes::RecordingInfo;
use saturating_cast::SaturatingCast as _;

use super::Layer;
use crate::Error;

/// Build the [`RecordingInfo`] chunk using the message statistics from a [`mcap::Summary`].
#[derive(Debug, Default)]
pub struct McapRecordingInfoLayer;

impl Layer for McapRecordingInfoLayer {
    fn identifier() -> super::LayerIdentifier {
        "recording_info".into()
    }

    fn process(
        &mut self,
        _mcap_bytes: &[u8],
        summary: &mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> std::result::Result<(), Error> {
        let properties = summary
            .stats
            .as_ref()
            .map(|s| {
                RecordingInfo::new().with_start_time(s.message_start_time.saturating_cast::<i64>())
            })
            .unwrap_or_default();

        let chunk = Chunk::builder(EntityPath::properties())
            .with_archetype(RowId::new(), TimePoint::STATIC, &properties)
            .build()?;

        emit(chunk);

        Ok(())
    }
}
