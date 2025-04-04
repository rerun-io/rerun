use custom_callback::{comms::viewer::ControlViewer, panel::Control};

use rerun::external::re_viewer::AsyncRuntimeHandle;
use rerun::external::{eframe, re_log, re_memory, re_viewer};

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
    let main_thread_token = re_viewer::MainThreadToken::i_promise_i_am_on_the_main_thread();
    // Direct calls using the `log` crate to stderr. Control with `RUST_LOG=debug` etc.
    re_log::setup_logging();

    // Install handlers for panics and crashes that prints to stderr and send
    // them to Rerun analytics (if the `analytics` feature is on in `Cargo.toml`).
    re_crash_handler::install_crash_handlers(re_viewer::build_info());

    // Listen for gRPC connections from Rerun's logging SDKs.
    // There are other ways of "feeding" the viewer though - all you need is a `re_smart_channel::Receiver`.
    let (rx_log, rx_table) = re_grpc_server::spawn_with_recv(
        "0.0.0.0:9877".parse()?,
        "75%".parse()?,
        re_grpc_server::shutdown::never(),
    );

    // First we attempt to connect to the external application
    let viewer = ControlViewer::connect(format!("127.0.0.1:{CONTROL_PORT}")).await?;
    let handle = viewer.handle();

    // Spawn the viewer client in a separate task
    tokio::spawn(async move {
        viewer.run().await;
    });

    // Then we start the Rerun viewer
    let mut native_options = re_viewer::native::eframe_options(None);
    native_options.viewport = native_options
        .viewport
        .with_app_id("rerun_example_custom_callback");

    // This is used for analytics, if the `analytics` feature is on in `Cargo.toml`
    let app_env = re_viewer::AppEnvironment::Custom("My Custom Callback".to_owned());

    let startup_options = re_viewer::StartupOptions::default();
    let window_title = "Rerun Control Panel";
    eframe::run_native(
        window_title,
        native_options,
        Box::new(move |cc| {
            re_viewer::customize_eframe_and_setup_renderer(cc)?;

            let mut rerun_app = re_viewer::App::new(
                main_thread_token,
                re_viewer::build_info(),
                &app_env,
                startup_options,
                cc,
                AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()?,
            );

            rerun_app.add_log_receiver(rx_log);
            rerun_app.add_table_receiver(rx_table);

            Ok(Box::new(Control::new(rerun_app, handle)))
        }),
    )?;

    Ok(())
}
