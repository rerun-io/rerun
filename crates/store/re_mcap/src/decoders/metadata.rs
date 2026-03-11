use std::collections::BTreeMap;

use re_chunk::{Chunk, EntityPath, RowId, TimePoint};
use re_sdk_types::{
    Component as _, ComponentBatch as _, ComponentDescriptor, SerializedComponentBatch, components,
    datatypes,
};

use super::{Decoder, DecoderIdentifier};
use crate::Error;

/// Extracts [`mcap::records::Metadata`] records from an MCAP file as a single static chunk.
///
/// Outputs a single `McapMetadata` archetype at [`EntityPath::properties()`],
/// with one [`components::KeyValuePairs`] component per metadata record.
#[derive(Debug, Default)]
pub struct McapMetadataDecoder;

const ARCHETYPE_NAME: &str = "McapMetadata";

impl Decoder for McapMetadataDecoder {
    fn identifier() -> DecoderIdentifier {
        "metadata".into()
    }

    fn process(
        &mut self,
        mcap_bytes: &[u8],
        summary: &mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> Result<(), Error> {
        if summary.metadata_indexes.is_empty() {
            return Ok(());
        }

        // We can encounter multiple metadata records with the same name.
        // Collect all metadata records by name, merging key-value pairs from records with the same name.
        let mut metadata_by_name: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

        for index in &summary.metadata_indexes {
            let metadata = match mcap::read::metadata(mcap_bytes, index) {
                Ok(metadata) => metadata,
                Err(err) => {
                    re_log::warn_once!(
                        "Failed to read MCAP metadata record '{}': {err}",
                        index.name
                    );
                    continue;
                }
            };

            re_log::debug!(
                "Processing MCAP metadata record '{}' with {} entries",
                metadata.name,
                metadata.metadata.len(),
            );

            let entries = metadata_by_name.entry(metadata.name.clone()).or_default();
            for (key, value) in &metadata.metadata {
                if entries.insert(key.clone(), value.clone()).is_some() {
                    re_log::warn_once!(
                        "Key '{key}' appears in multiple MCAP metadata records named '{}'",
                        metadata.name
                    );
                }
            }
        }

        let mut batches: Vec<SerializedComponentBatch> = Vec::new();
        for (name, entries) in metadata_by_name {
            let pairs: Vec<_> = entries
                .into_iter()
                .map(|(key, value)| datatypes::Utf8Pair {
                    first: key.into(),
                    second: value.into(),
                })
                .collect();
            let kv = components::KeyValuePairs(pairs);
            batches.push(kv.try_serialized(ComponentDescriptor {
                archetype: Some(ARCHETYPE_NAME.into()),
                component: name.into(),
                component_type: Some(components::KeyValuePairs::name()),
            })?);
        }

        if !batches.is_empty() {
            let chunk = Chunk::builder(EntityPath::properties())
                .with_serialized_batches(RowId::new(), TimePoint::STATIC, batches)
                .build()?;
            emit(chunk);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io;

    use re_chunk::Chunk;

    use crate::DecoderRegistry;

    use super::*;

    /// Helper function to run the metadata decoder and collect emitted chunks.
    fn run_metadata_decoder(buffer: &[u8]) -> Vec<Chunk> {
        let reader = io::Cursor::new(buffer);
        let summary = crate::read_summary(reader)
            .expect("failed to read summary")
            .expect("no summary found");

        let mut chunks = Vec::new();
        let registry = DecoderRegistry::empty().register_file_decoder::<McapMetadataDecoder>();
        registry
            .plan(&summary)
            .expect("failed to plan")
            .run(buffer, &summary, &mut |chunk| chunks.push(chunk))
            .expect("failed to run decoder");
        chunks
    }

    /// Tests that multiple metadata records are merged into a single chunk with one component per metadata.
    #[test]
    fn test_multiple_metadata_records() {
        let buffer = {
            let cursor = io::Cursor::new(Vec::new());
            let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

            for i in 0..3 {
                let mut key_values = BTreeMap::new();
                key_values.insert("index".to_owned(), i.to_string());
                writer
                    .write_metadata(&mcap::records::Metadata {
                        name: format!("meta_{i}"),
                        metadata: key_values,
                    })
                    .expect("failed to write metadata");
            }

            writer.finish().expect("failed to finish writer");
            writer.into_inner().into_inner()
        };

        let chunks = run_metadata_decoder(&buffer);
        assert_eq!(chunks.len(), 1, "all metadata in a single chunk");

        let chunk = &chunks[0];
        assert_eq!(chunk.entity_path(), &EntityPath::properties());
        assert!(chunk.is_static());
        assert_eq!(chunk.num_components(), 3);
    }

    /// Tests that two metadata records with the same name are merged into one component.
    #[test]
    fn test_duplicate_metadata_names() {
        let buffer = {
            const METADATA_NAME: &str = "duplicated_metadata_name";
            let cursor = io::Cursor::new(Vec::new());
            let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

            let mut first = BTreeMap::new();
            first.insert("key_a".to_owned(), "value_a".to_owned());
            writer
                .write_metadata(&mcap::records::Metadata {
                    name: METADATA_NAME.to_owned(),
                    metadata: first,
                })
                .expect("failed to write metadata");

            let mut second = BTreeMap::new();
            second.insert("key_b".to_owned(), "value_b".to_owned());
            writer
                .write_metadata(&mcap::records::Metadata {
                    name: METADATA_NAME.to_owned(),
                    metadata: second,
                })
                .expect("failed to write metadata");

            writer.finish().expect("failed to finish writer");
            writer.into_inner().into_inner()
        };

        let chunks = run_metadata_decoder(&buffer);
        assert_eq!(chunks.len(), 1);

        let chunk = &chunks[0];
        assert_eq!(
            chunk.num_components(),
            1,
            "duplicates merged into one component"
        );
    }
}
