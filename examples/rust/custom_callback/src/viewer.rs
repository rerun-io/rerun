use custom_callback::{comms::viewer::ControlViewer, panel::Control};

use re_viewer::external::{re_log, re_memory};
use std::net::Ipv4Addr;

// By using `re_memory::AccountingAllocator` Rerun can keep track of exactly how much memory it is using,
// and prune the data store when it goes above a certain limit.
// By using `mimalloc` we get faster allocations.
#[global_allocator]
static GLOBAL: re_memory::AccountingAllocator<mimalloc::MiMalloc> =
    re_memory::AccountingAllocator::new(mimalloc::MiMalloc);

/// Port used for control messages
const CONTROL_PORT: u16 = 8888;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Direct calls using the `log` crate to stderr. Control with `RUST_LOG=debug` etc.
    re_log::setup_logging();

    // Install handlers for panics and crashes that prints to stderr and send
    // them to Rerun analytics (if the `analytics` feature is on in `Cargo.toml`).
    re_crash_handler::install_crash_handlers(re_viewer::build_info());

    // Listen for TCP connections from Rerun's logging SDKs.
    // There are other ways of "feeding" the viewer though - all you need is a `re_smart_channel::Receiver`.
    let rx = re_sdk_comms::serve(
        &Ipv4Addr::UNSPECIFIED.to_string(),
        re_sdk_comms::DEFAULT_SERVER_PORT + 1,
        Default::default(),
    )?;

    let startup_options = re_viewer::StartupOptions::default();
    let app_env = re_viewer::AppEnvironment::Custom("Rerun Control Panel".to_owned());
    let viewer = ControlViewer::connect(format!("127.0.0.1:{CONTROL_PORT}")).await?;

    let handle = viewer.handle();

    // Spawn the viewer in a separate task
    tokio::spawn(async move {
        viewer.run().await;
    });

    re_viewer::run_native_app(
        Box::new(move |cc| {
            let mut app = re_viewer::App::new(
                re_viewer::build_info(),
                &app_env,
                startup_options,
                cc.egui_ctx.clone(),
                cc.storage,
            );
            app.add_receiver(rx);
            Box::new(Control::new(app, handle))
        }),
        None,
    )?;

    Ok(())
}
