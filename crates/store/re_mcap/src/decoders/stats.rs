use re_chunk::{Chunk, EntityPath, RowId, TimePoint};
use re_sdk_types::archetypes::McapStatistics;
use re_sdk_types::{components, datatypes};
use saturating_cast::SaturatingCast as _;

use super::{Decoder, DecoderContext, DecoderIdentifier, MCAP_PROPERTIES_ENTITY_PATH};
use crate::Error;

/// Extracts [`mcap::records::Statistics`], such as message count, from an MCAP file.
///
/// The results will be stored at `__mcap_properties`.
#[derive(Debug, Default)]
pub struct McapStatisticDecoder;

impl Decoder for McapStatisticDecoder {
    fn identifier() -> DecoderIdentifier {
        "stats".into()
    }

    fn process(
        &mut self,
        ctx: &DecoderContext<'_>,
        emit: &(dyn Fn(Chunk) + Send + Sync),
    ) -> Result<(), Error> {
        if let Some(statistics) = ctx.summary().stats.as_ref() {
            let chunk = Chunk::builder(EntityPath::from(MCAP_PROPERTIES_ENTITY_PATH))
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &from_statistics(statistics),
                )
                .build()?;
            emit(chunk);
        } else {
            re_log::warn_once!("Could not access MCAP statistics information.");
        }

        Ok(())
    }
}

fn from_statistics(stats: &::mcap::records::Statistics) -> McapStatistics {
    let ::mcap::records::Statistics {
        message_count,
        schema_count,
        channel_count,
        attachment_count,
        metadata_count,
        chunk_count,
        message_start_time,
        message_end_time,
        channel_message_counts,
    } = stats;

    let channel_count_pairs: Vec<_> = channel_message_counts
        .iter()
        .map(|(&channel_id, &count)| datatypes::ChannelCountPair {
            channel_id: channel_id.into(),
            message_count: count.into(),
        })
        .collect();

    McapStatistics::update_fields()
        .with_message_count(*message_count)
        .with_schema_count(*schema_count as u64)
        .with_channel_count(*channel_count as u64)
        .with_attachment_count(*attachment_count as u64)
        .with_metadata_count(*metadata_count as u64)
        .with_chunk_count(*chunk_count as u64)
        .with_message_start_time(message_start_time.saturating_cast::<i64>())
        .with_message_end_time(message_end_time.saturating_cast::<i64>())
        .with_channel_message_counts(components::ChannelMessageCounts(channel_count_pairs))
}

#[cfg(test)]
mod tests {
    use std::io;

    use re_chunk::Chunk;
    use re_log_types::TimeType;

    use crate::DecoderRegistry;
    use crate::decoders::TestEmitter;

    use super::*;

    fn run_stats_decoder(buffer: &[u8]) -> Vec<Chunk> {
        let reader = io::Cursor::new(buffer);
        let summary = crate::read_summary(reader)
            .expect("failed to read summary")
            .expect("no summary found");

        let emitter = TestEmitter::default();
        let registry = DecoderRegistry::empty().register_file_decoder::<McapStatisticDecoder>();
        registry
            .plan(buffer, &summary, &crate::TopicFilter::default())
            .expect("failed to plan")
            .run(buffer, &summary, TimeType::TimestampNs, &*emitter)
            .expect("failed to run decoder");
        emitter.finish()
    }

    #[test]
    fn test_stats_entity_path() {
        let buffer = {
            let cursor = io::Cursor::new(Vec::new());
            let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");
            writer.finish().expect("failed to finish writer");
            writer.into_inner().into_inner()
        };

        let chunks = run_stats_decoder(&buffer);
        assert_eq!(chunks.len(), 1);

        let chunk = &chunks[0];
        assert_eq!(
            chunk.entity_path(),
            &EntityPath::from(MCAP_PROPERTIES_ENTITY_PATH)
        );
        assert!(chunk.is_static());
    }
}
