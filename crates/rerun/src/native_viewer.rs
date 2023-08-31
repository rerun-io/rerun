use re_log_types::LogMsg;
use re_log_types::StoreInfo;
use re_sdk::RecordingStream;

/// Starts a Rerun viewer on the current thread and migrates the given callback, along with
/// the active `RecordingStream`, to a newly spawned thread where the callback will run until
/// completion.
///
/// All messages logged from the passed-in callback will be streamed to the viewer in
/// real-time.
///
/// The method will return when the viewer is closed.
///
/// ⚠️  This function must be called from the main thread since some platforms require that
/// their UI runs on the main thread! ⚠️
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn<F>(
    store_info: StoreInfo,
    batcher_config: re_log_types::DataTableBatcherConfig,
    run: F,
) -> re_viewer::external::eframe::Result<()>
where
    F: FnOnce(RecordingStream) + Send + 'static,
{
    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::Sdk,
        re_smart_channel::SmartChannelSource::Sdk,
    );
    let sink = Box::new(NativeViewerSink(tx));
    let app_env = re_viewer::AppEnvironment::from_store_source(&store_info.store_source);

    let rec =
        RecordingStream::new(store_info, batcher_config, sink).expect("Failed to spawn thread");

    // NOTE: Forget the handle on purpose, leave that thread be.
    std::thread::Builder::new()
        .name("spawned".into())
        .spawn(move || run(rec))
        .expect("Failed to spawn thread");

    // NOTE: Some platforms still mandate that the UI must run on the main thread, so make sure
    // to spawn the viewer in place and migrate the user callback to a new thread.
    re_viewer::run_native_app(Box::new(move |cc, re_ui| {
        let startup_options = re_viewer::StartupOptions::default();
        let mut app = re_viewer::App::new(
            re_build_info::build_info!(),
            &app_env,
            startup_options,
            re_ui,
            cc.storage,
        );
        app.add_receiver(rx);
        Box::new(app)
    }))
}

/// Starts a Rerun viewer to visualize the contents of a given array of messages.
/// The method will return when the viewer is closed.
///
/// ⚠️  This function must be called from the main thread since some platforms require that
/// their UI runs on the main thread! ⚠️
pub fn show(msgs: Vec<LogMsg>) -> re_viewer::external::eframe::Result<()> {
    if msgs.is_empty() {
        re_log::debug!("Empty array of msgs - call to show() ignored");
        return Ok(());
    }

    let store_source = re_log_types::StoreSource::RustSdk {
        rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
        llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
    };

    let startup_options = re_viewer::StartupOptions::default();
    re_viewer::run_native_viewer_with_messages(
        re_build_info::build_info!(),
        re_viewer::AppEnvironment::from_store_source(&store_source),
        startup_options,
        msgs,
    )
}

// ----------------------------------------------------------------------------

/// Stream log messages to a native viewer on the main thread.
#[cfg(feature = "native_viewer")]
struct NativeViewerSink(pub re_smart_channel::Sender<LogMsg>);

#[cfg(feature = "native_viewer")]
impl re_sdk::sink::LogSink for NativeViewerSink {
    fn send(&self, msg: LogMsg) {
        if let Err(err) = self.0.send(msg) {
            re_log::error_once!("Failed to send log message to viewer: {err}");
        }
    }

    #[inline]
    fn flush_blocking(&self) {}
}
