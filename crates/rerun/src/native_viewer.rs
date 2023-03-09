use re_log_types::LogMsg;
use re_log_types::RecordingInfo;
use re_sdk::Session;

/// Starts a Rerun viewer on the current thread and migrates the given callback, along with
/// the active `Session`, to a newly spawned thread where the callback will run until
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
pub fn spawn<F>(recording_info: RecordingInfo, run: F) -> re_viewer::external::eframe::Result<()>
where
    F: FnOnce(Session) + Send + 'static,
{
    let (tx, rx) = re_smart_channel::smart_channel(re_smart_channel::Source::Sdk);
    let sink = Box::new(NativeViewerSink(tx));
    let app_env =
        re_viewer::AppEnvironment::from_recording_source(&recording_info.recording_source);

    let session = Session::new(recording_info, sink);

    // NOTE: Forget the handle on purpose, leave that thread be.
    std::thread::Builder::new()
        .name("spawned".into())
        .spawn(move || run(session))
        .expect("Failed to spawn thread");

    // NOTE: Some platforms still mandate that the UI must run on the main thread, so make sure
    // to spawn the viewer in place and migrate the user callback to a new thread.
    re_viewer::run_native_app(Box::new(move |cc, re_ui| {
        // TODO(cmc): it'd be nice to centralize all the UI wake up logic somewhere.
        let rx = re_viewer::wake_up_ui_thread_on_each_msg(rx, cc.egui_ctx.clone());
        let startup_options = re_viewer::StartupOptions::default();
        Box::new(re_viewer::App::from_receiver(
            re_build_info::build_info!(),
            &app_env,
            startup_options,
            re_ui,
            cc.storage,
            rx,
        ))
    }))
}

/// Drains all pending log messages and starts a Rerun viewer to visualize
/// everything that has been logged so far.
///
/// The method will return when the viewer is closed.
///
/// You should use this method together with [`Session::buffered`];
///
/// ⚠️  This function must be called from the main thread since some platforms require that
/// their UI runs on the main thread! ⚠️
pub fn show(session: &Session) -> re_viewer::external::eframe::Result<()> {
    if !session.is_enabled() {
        re_log::debug!("Rerun disabled - call to show() ignored");
        return Ok(());
    }
    let recording_source = re_log_types::RecordingSource::RustSdk {
        rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
        llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
    };

    let log_messages = session.drain_backlog();
    let startup_options = re_viewer::StartupOptions::default();
    re_viewer::run_native_viewer_with_messages(
        re_build_info::build_info!(),
        re_viewer::AppEnvironment::from_recording_source(&recording_source),
        startup_options,
        log_messages,
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
}
