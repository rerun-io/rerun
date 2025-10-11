use re_chunk_store::ChunkStoreConfig;
use re_server::{RerunCloudHandler, RerunCloudHandlerBuilder};

#[expect(clippy::unused_async)] // needed by the macro
async fn build() -> RerunCloudHandler {
    // SAFETY: it's a test
    #[expect(unsafe_code)]
    unsafe {
        // Mimic the behavior of cloud
        std::env::set_var(ChunkStoreConfig::ENV_CHUNK_MAX_BYTES, "0");
        std::env::set_var(ChunkStoreConfig::ENV_CHUNK_MAX_ROWS, "0");
        std::env::set_var(ChunkStoreConfig::ENV_CHUNK_MAX_ROWS_IF_UNSORTED, "0");
    }

    RerunCloudHandlerBuilder::new().build()
}

re_redap_tests::generate_redap_tests!(build);
