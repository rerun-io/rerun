use std::sync::Arc;

use arrow::{
    array::{MapBuilder, UInt16Array, UInt16Builder, UInt32Array, UInt64Array, UInt64Builder},
    error::ArrowError,
};
use re_chunk::{Chunk, EntityPath, RowId, TimePoint};
use re_types::{AnyValues, components};

use crate::mcap::decode::PluginError;

use super::{LayerIdentifier, Layer};

/// Send the statistics as recording properties.
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
    ) -> Result<(), PluginError> {
        if let Some(statistics) = summary.stats.as_ref() {
            let chunk = Chunk::builder(EntityPath::properties())
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &from_statistics(statistics)?,
                )
                .build()?;
            emit(chunk);
        } else {
            re_log::warn_once!("Could not access MCAP statistics information.");
        }

        Ok(())
    }
}

fn from_statistics(stats: &::mcap::records::Statistics) -> Result<AnyValues, ArrowError> {
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

    let key_builder = UInt16Builder::new();
    let val_builder = UInt64Builder::new();

    let mut builder = MapBuilder::new(None, key_builder, val_builder);

    for (&key, &val) in channel_message_counts {
        builder.keys().append_value(key);
        builder.values().append_value(val);
        builder.append(true)?;
    }

    let channel_message_counts = builder.finish();

    Ok(AnyValues::new("rerun.mcap.Statistics")
        .with_field(
            "message_count",
            Arc::new(UInt64Array::from_value(*message_count, 1)),
        )
        .with_field(
            "schema_count",
            Arc::new(UInt16Array::from_value(*schema_count, 1)),
        )
        .with_field(
            "channel_count",
            Arc::new(UInt32Array::from_value(*channel_count, 1)),
        )
        .with_field(
            "attachment_count",
            Arc::new(UInt32Array::from_value(*attachment_count, 1)),
        )
        .with_field(
            "metadata_count",
            Arc::new(UInt32Array::from_value(*metadata_count, 1)),
        )
        .with_field(
            "chunk_count",
            Arc::new(UInt32Array::from_value(*chunk_count, 1)),
        )
        .with_component::<components::Timestamp>(
            "message_start_time",
            vec![*message_start_time as i64],
        )
        .with_component::<components::Timestamp>("message_end_time", vec![*message_end_time as i64])
        .with_field("channel_message_counts", Arc::new(channel_message_counts)))
}
