#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk::Chunk;
    use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
    use re_data_loader::loader_mcap::load_mcap;
    use re_data_loader::{DataLoaderSettings, LoadedData};
    use re_log_types::StoreId;
    use re_mcap::layers::SelectedLayers;

    // Load an MCAP file into a list of chunks.
    fn load_mcap_chunks(path: impl AsRef<std::path::Path>) -> Vec<Chunk> {
        let path = path.as_ref();
        println!("Loading MCAP file: {}", path.display());
        let mcap_data = std::fs::read(path).unwrap();
        let (tx, rx) = crossbeam::channel::bounded(1024);
        let settings = DataLoaderSettings::recommended("test");
        load_mcap(
            &mcap_data,
            &settings,
            &tx,
            &SelectedLayers::All,
            false,
            None,
        )
        .unwrap();
        drop(tx);

        // Collect chunks
        rx.iter()
            .filter_map(|res| {
                if let LoadedData::Chunk(_, _, chunk) = res {
                    Some(chunk)
                } else {
                    None
                }
            })
            .collect()
    }

    // TODO(grtlr): This should be something like a snippet / backwards-compatibility test, but
    // we don't really have the infrastructure for this yet and we already test a different
    // MCAP file in snippets.
    #[test]
    fn test_mcap_loader_ros2() {
        let chunks = load_mcap_chunks("tests/assets/supported_ros2_messages.mcap");

        // Create a ChunkStore and ChunkStoreHandle
        let store = ChunkStore::new(
            StoreId::random(re_log_types::StoreKind::Recording, "test_mcap_loader"),
            ChunkStoreConfig::default(),
        );
        let store_handle = ChunkStoreHandle::new(store);

        // Insert all chunks into the store
        {
            let mut store = store_handle.write();
            for chunk in chunks {
                store.insert_chunk(&Arc::new(chunk)).unwrap();
            }
        }

        // Extract and snapshot the schema
        let schema = store_handle.read().schema();
        insta::assert_debug_snapshot!("ros2", schema);
    }
}
