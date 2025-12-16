use re_chunk::{Chunk, EntityPath, RowId, TimePoint};
use re_sdk_types::archetypes::McapStatistics;
use re_sdk_types::{components, datatypes};
use saturating_cast::SaturatingCast as _;

use super::{Layer, LayerIdentifier};
use crate::Error;

/// Extracts [`mcap::records::Statistics`], such as message count, from an MCAP file.
///
/// The results will be stored as recording properties.
#[derive(Debug, Default)]
pub struct McapStatisticLayer;

impl Layer for McapStatisticLayer {
    fn identifier() -> LayerIdentifier {
        "stats".into()
    }

    fn process(
        &mut self,
        _mcap_bytes: &[u8],
        summary: &mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> Result<(), Error> {
        if let Some(statistics) = summary.stats.as_ref() {
            let chunk = Chunk::builder(EntityPath::properties())
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
