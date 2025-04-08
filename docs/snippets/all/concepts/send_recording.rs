//! Send a `.rrd` to a new recording stream.

use rerun::external::re_chunk_store::{ChunkStore, ChunkStoreConfig};
use rerun::VersionPolicy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the filename from the command-line args.
    let filename = std::env::args().nth(2).ok_or("Missing filename argument")?;

    // Load the chunk store from the file.
    let (store_id, store) =
        ChunkStore::from_rrd_filepath(&ChunkStoreConfig::DEFAULT, filename, VersionPolicy::Warn)?
            .into_iter()
            .next()
            .ok_or("Expected exactly one recording in the archive")?;

    // Use the same app and recording IDs as the original.
    if let Some(info) = store.info().cloned() {
        let new_recording = rerun::RecordingStreamBuilder::new(info.application_id.clone())
            .store_id(store_id)
            .spawn()?;

        // Forward all chunks to the new recording stream.
        for chunk in store.iter_chunks() {
            new_recording.send_chunk((**chunk).clone());
        }
    }

    Ok(())
}
