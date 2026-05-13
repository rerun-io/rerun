use re_chunk::{Chunk, EntityPath, RowId, TimePoint};
use re_sdk_types::archetypes::RecordingInfo;
use saturating_cast::SaturatingCast as _;

use super::{Decoder, DecoderContext, MCAP_PROPERTIES_ENTITY_PATH};
use crate::Error;

/// Build the [`RecordingInfo`] chunk using the message statistics from a [`mcap::Summary`].
#[derive(Debug, Default)]
pub struct McapRecordingInfoDecoder;

impl Decoder for McapRecordingInfoDecoder {
    fn identifier() -> super::DecoderIdentifier {
        "recording_info".into()
    }

    fn process(
        &mut self,
        ctx: &DecoderContext<'_>,
        emit: &(dyn Fn(Chunk) + Send + Sync),
    ) -> std::result::Result<(), Error> {
        let properties = ctx
            .summary()
            .stats
            .as_ref()
            .map(|s| {
                RecordingInfo::new().with_start_time(s.message_start_time.saturating_cast::<i64>())
            })
            .unwrap_or_default();

        let chunk = Chunk::builder(EntityPath::from(MCAP_PROPERTIES_ENTITY_PATH))
            .with_archetype(RowId::new(), TimePoint::STATIC, &properties)
            .build()?;

        emit(chunk);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use re_chunk::Chunk;
    use re_log_types::TimeType;

    use crate::DecoderRegistry;
    use crate::decoders::TestEmitter;

    use super::*;

    fn run_recording_info_decoder(buffer: &[u8]) -> Vec<Chunk> {
        let reader = io::Cursor::new(buffer);
        let summary = crate::read_summary(reader)
            .expect("failed to read summary")
            .expect("no summary found");

        let emitter = TestEmitter::default();
        let registry = DecoderRegistry::empty().register_file_decoder::<McapRecordingInfoDecoder>();
        registry
            .plan(buffer, &summary, &crate::TopicFilter::default())
            .expect("failed to plan")
            .run(buffer, &summary, TimeType::TimestampNs, &*emitter)
            .expect("failed to run decoder");
        emitter.finish()
    }

    #[test]
    fn test_recording_info_entity_path() {
        let buffer = {
            let cursor = io::Cursor::new(Vec::new());
            let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");
            writer.finish().expect("failed to finish writer");
            writer.into_inner().into_inner()
        };

        let chunks = run_recording_info_decoder(&buffer);
        assert_eq!(chunks.len(), 1);

        let chunk = &chunks[0];
        assert_eq!(
            chunk.entity_path(),
            &EntityPath::from(MCAP_PROPERTIES_ENTITY_PATH)
        );
        assert!(chunk.is_static());
    }
}
