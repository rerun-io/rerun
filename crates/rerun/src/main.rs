use re_memory::AccountingAllocator;

#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();
    rerun::run(std::env::args()).await
}

// TODO:
//
// start_method:
// - viewer-is-a-server
// - viewer-connects-to-a-server
// - server (native|web?)
// - load_rrd
// - show()
//
// application_id / record_id: gotta grab em from the initial LogMsg
//
// - [x] rerun_version
// - [x] rust_version
// - [x] target_platform
// - [ ] start_method
// - [x] application_id
// - [x] recording_id
// - [x] recording_source
// - [ ] recording_source_is_remote (?)
//         most likely superseded by start_method..?
// - [x] is_official_example
