#[cfg(test)]
mod tests {
    use re_chunk::{Chunk, ChunkId};
    use re_data_loader::{DataLoaderSettings, LoadedData, loader_mcap::load_mcap};
    use re_mcap::layers::SelectedLayers;

    // Load an MCAP file into a list of chunks.
    fn load_mcap_chunks(path: &std::path::Path) -> Vec<Chunk> {
        println!("Loading MCAP file: {}", path.display());
        let mcap_data = std::fs::read(path).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let settings = DataLoaderSettings::recommended("test");
        load_mcap(&mcap_data, &settings, &tx, &SelectedLayers::All, false).unwrap();
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

    // Compare chunks based on their debug representation.
    // Chunks are sorted by entity path and row ids are cleared to make comparison stable.
    fn assert_chunk_snapshot(mut chunks: Vec<Chunk>) {
        chunks.sort_by_key(|chunk| chunk.entity_path().to_string());
        let clean_chunks: Vec<Chunk> = chunks
            .into_iter()
            .map(|chunk| {
                chunk
                    .with_id(ChunkId::from_u128(123_456_789_123_456_789_123_456_789))
                    .zeroed()
            })
            .collect();
        insta::assert_debug_snapshot!(clean_chunks);
    }

    #[test]
    fn test_mcap_loader() {
        insta::glob!("assets/*.mcap", |path| {
            let chunks = load_mcap_chunks(path);
            assert_chunk_snapshot(chunks);
        });
    }
}
