use re_chunk::{Chunk, EntityPath, RowId, TimePoint};
use re_types::archetypes::RecordingInfo;

use crate::mcap::decode::PluginError;

use super::LayerNew;

/// Build the [`RecordingInfo`] chunk using the message statistics from a [`Summary`].
#[derive(Debug, Default)]
pub struct McapRecordingInfoLayer;

impl LayerNew for McapRecordingInfoLayer {
    fn identifier() -> super::LayerIdentifier {
        "recording_info".into()
    }

    fn process(
        &mut self,
        _mcap_bytes: &[u8],
        summary: &mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> std::result::Result<(), PluginError> {
        let properties = summary
            .stats
            .as_ref()
            .map(|s| RecordingInfo::new().with_start_time(s.message_start_time as i64))
            .unwrap_or_default();

        let chunk = Chunk::builder(EntityPath::properties())
            .with_archetype(RowId::new(), TimePoint::STATIC, &properties)
            .build()?;

        emit(chunk);

        Ok(())
    }
}
