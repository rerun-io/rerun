use re_chunk::{Chunk, RowId, TimePoint};
use re_sdk_types::{
    Component as _, ComponentBatch as _, ComponentDescriptor, SerializedComponentBatch, components,
    datatypes,
};

use super::{Decoder, DecoderContext, DecoderIdentifier};
use crate::Error;

/// Extracts [`mcap::Attachment`] records from an MCAP file as static chunks.
///
/// Outputs one `McapAttachment` entity at `__mcap_attachments` with all attachments.
#[derive(Debug, Default)]
pub struct McapAttachmentsDecoder;

const ARCHETYPE_NAME: &str = "McapAttachment";
const MCAP_ATTACHMENTS_ENTITY_PATH: &str = "__mcap_attachments";

impl Decoder for McapAttachmentsDecoder {
    fn identifier() -> DecoderIdentifier {
        "attachments".into()
    }

    fn process(
        &mut self,
        ctx: &DecoderContext<'_>,
        emit: &(dyn Fn(Chunk) + Send + Sync),
    ) -> Result<(), Error> {
        if ctx.summary().attachment_indexes.is_empty() {
            return Ok(());
        }

        let mut attachments = Vec::new();

        for (index, attachment) in ctx.attachment_records() {
            let attachment = match attachment {
                Ok(attachment) => attachment,
                Err(err) => {
                    re_log::warn_once!(
                        "Failed to read MCAP attachment record '{}': {err}",
                        index.name
                    );
                    continue;
                }
            };

            re_log::debug!(
                "Processing MCAP attachment '{}' with media type '{}' and {} bytes",
                attachment.name,
                attachment.media_type,
                attachment.data.len(),
            );

            attachments.push((index, attachment));
        }

        if !attachments.is_empty() {
            let chunk = Chunk::builder(MCAP_ATTACHMENTS_ENTITY_PATH)
                .with_serialized_batches(
                    RowId::new(),
                    TimePoint::STATIC,
                    attachments_batches(&attachments)?,
                )
                .build()?;
            emit(chunk);
        }

        Ok(())
    }
}

fn attachments_batches(
    attachments: &[(&mcap::records::AttachmentIndex, mcap::Attachment<'_>)],
) -> Result<Vec<SerializedComponentBatch>, Error> {
    let data = attachments
        .iter()
        .map(|(_, attachment)| components::Blob(attachment.data.as_ref().to_vec().into()))
        .collect::<Vec<_>>();
    let media_types = attachments
        .iter()
        .map(|(_, attachment)| components::MediaType(attachment.media_type.clone().into()))
        .collect::<Vec<_>>();
    let metadata = attachments
        .iter()
        .map(|(index, attachment)| {
            components::KeyValuePairs(vec![
                datatypes::Utf8Pair {
                    first: "name".into(),
                    second: attachment.name.clone().into(),
                },
                datatypes::Utf8Pair {
                    first: "media_type".into(),
                    second: attachment.media_type.clone().into(),
                },
                datatypes::Utf8Pair {
                    first: "log_time".into(),
                    second: attachment.log_time.to_string().into(),
                },
                datatypes::Utf8Pair {
                    first: "create_time".into(),
                    second: attachment.create_time.to_string().into(),
                },
                datatypes::Utf8Pair {
                    first: "data_size".into(),
                    second: attachment.data.len().to_string().into(),
                },
                datatypes::Utf8Pair {
                    first: "offset".into(),
                    second: index.offset.to_string().into(),
                },
            ])
        })
        .collect::<Vec<_>>();

    Ok(vec![
        data.try_serialized(ComponentDescriptor {
            archetype: Some(ARCHETYPE_NAME.into()),
            component: "data".into(),
            component_type: Some(components::Blob::name()),
        })?,
        media_types.try_serialized(ComponentDescriptor {
            archetype: Some(ARCHETYPE_NAME.into()),
            component: "media_type".into(),
            component_type: Some(components::MediaType::name()),
        })?,
        metadata.try_serialized(ComponentDescriptor {
            archetype: Some(ARCHETYPE_NAME.into()),
            component: "metadata".into(),
            component_type: Some(components::KeyValuePairs::name()),
        })?,
    ])
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::io;

    use re_chunk::Chunk;
    use re_chunk::EntityPath;
    use re_chunk::external::arrow::array::Array as _;

    use re_log_types::TimeType;

    use crate::DecoderRegistry;
    use crate::decoders::TestEmitter;

    use super::*;

    /// Test helper for decoding test MCAP attachments from bytes.
    fn run_attachments_decoder(buffer: &[u8]) -> Vec<Chunk> {
        let reader = io::Cursor::new(buffer);
        let summary = crate::read_summary(reader)
            .expect("failed to read summary")
            .expect("no summary found");

        let emitter = TestEmitter::default();
        let registry = DecoderRegistry::empty().register_file_decoder::<McapAttachmentsDecoder>();
        registry
            .plan(buffer, &summary, &crate::TopicFilter::default())
            .expect("failed to plan")
            .run(buffer, &summary, TimeType::TimestampNs, &*emitter)
            .expect("failed to run decoder");
        emitter.finish()
    }

    /// Test helper for serializing MCAP attachments to bytes.
    fn attachments_buffer(attachments: &[mcap::Attachment<'_>]) -> Vec<u8> {
        let cursor = io::Cursor::new(Vec::new());
        let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

        for attachment in attachments {
            writer
                .attach(attachment)
                .expect("failed to write attachment");
        }

        writer.finish().expect("failed to finish writer");
        writer.into_inner().into_inner()
    }

    /// Tests that an MCAP attachment record is decoded.
    #[test]
    fn test_attachment_record() {
        let buffer = attachments_buffer(&[mcap::Attachment {
            log_time: 0,
            create_time: 1,
            name: "calibration".to_owned(),
            media_type: "application/json".to_owned(),
            data: Cow::Borrowed(b"{\"foo\":42}"),
        }]);

        let chunks = run_attachments_decoder(&buffer);
        assert_eq!(chunks.len(), 1);

        let chunk = &chunks[0];
        assert_eq!(
            chunk.entity_path(),
            &EntityPath::from(MCAP_ATTACHMENTS_ENTITY_PATH)
        );
        assert!(chunk.is_static());
        assert_eq!(chunk.num_components(), 3);
        assert_eq!(num_attachment_instances(chunk), 1);

        let mut descriptors = chunk
            .component_descriptors()
            .map(|descr| descr.component.to_string())
            .collect::<Vec<_>>();
        descriptors.sort();
        assert_eq!(descriptors, ["data", "media_type", "metadata"]);
    }

    /// Tests that all MCAP attachments are preserved independent of name.
    /// MCAP attachments don't enforce unique names per attachment.
    #[test]
    fn test_duplicate_attachment_names() {
        let buffer = attachments_buffer(&[
            mcap::Attachment {
                log_time: 0,
                create_time: 1,
                name: "calibration".to_owned(),
                media_type: "application/octet-stream".to_owned(),
                data: Cow::Borrowed(b"first"),
            },
            mcap::Attachment {
                log_time: 1,
                create_time: 2,
                name: "calibration".to_owned(),
                media_type: "application/octet-stream".to_owned(),
                data: Cow::Borrowed(b"second"),
            },
        ]);

        let chunks = run_attachments_decoder(&buffer);

        // We expect a single chunk at `__mcap_attachments` with two attachments.
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0].entity_path(),
            &EntityPath::from(MCAP_ATTACHMENTS_ENTITY_PATH)
        );
        assert_eq!(num_attachment_instances(&chunks[0]), 2);
    }

    /// Tests the attachments decoder against an MCAP file fixture.
    #[test]
    fn test_attachments_mcap_fixture() {
        let buffer = include_bytes!("assets/attachments.mcap");
        let chunks = run_attachments_decoder(buffer);
        assert_eq!(chunks.len(), 1);

        insta::assert_snapshot!(format_chunk(&chunks[0]));
    }

    fn num_attachment_instances(chunk: &Chunk) -> i32 {
        let mut arrays = chunk
            .components()
            .get_by_component_type(components::Blob::name());
        let Some(data) = arrays.next() else {
            panic!("missing attachment data component");
        };
        assert!(arrays.next().is_none());
        assert_eq!(data.len(), 1);
        data.value_length(0)
    }

    fn format_chunk(chunk: &Chunk) -> String {
        let batch = chunk.to_record_batch().expect("failed to convert chunk");
        re_arrow_util::RecordBatchFormatOpts {
            width: Some(240),
            max_cell_content_width: usize::MAX,
            redact_non_deterministic: true,
            ..Default::default()
        }
        .format(&batch)
        .to_string()
    }
}
