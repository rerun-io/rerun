use custom_callback::comms::viewer::ControlViewer;
use custom_callback::panel::Control;
use rerun::external::{eframe, re_crash_handler, re_log, re_memory, re_viewer};

// By using `re_memory::AccountingAllocator` Rerun can keep track of exactly how much memory it is using,
// and prune the data store when it goes above a certain limit.
// By using `mimalloc` we get faster allocations.
#[global_allocator]
static GLOBAL: re_memory::AccountingAllocator<mimalloc::MiMalloc> =
    re_memory::AccountingAllocator::new(mimalloc::MiMalloc);

/// Both served by `custom_callback_app`.
const RECORDING_URL: &str = "rerun+http://127.0.0.1:9876/proxy";
const CONTROL_URL: &str = "ws://127.0.0.1:9091/ws";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_thread_token = re_viewer::MainThreadToken::i_promise_i_am_on_the_main_thread();
    // Direct calls using the `log` crate to stderr. Control with `RUST_LOG=debug` etc.
    re_log::setup_logging();

    // Install handlers for panics and crashes that prints to stderr and send
    // them to Rerun analytics (if the `analytics` feature is on in `Cargo.toml`).
    re_crash_handler::install_crash_handlers(re_viewer::build_info());

    // Connect to the external application in the background, retrying until it is up.
    let viewer = ControlViewer::new(CONTROL_URL);
    let handle = viewer.handle();
    tokio::spawn(viewer.run());

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

            let rerun_app = re_viewer::App::new(
                main_thread_token,
                re_viewer::build_info(),
                app_env,
                startup_options,
                cc,
                None,
                re_viewer::AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()?,
            );

            rerun_app.open_url_or_file(RECORDING_URL);

            Ok(Box::new(Control::new(rerun_app, handle)))
        }),
    )?;

    Ok(())
}
