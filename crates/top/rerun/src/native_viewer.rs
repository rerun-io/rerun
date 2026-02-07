use re_log_types::LogMsg;
use re_viewer::external::eframe;

/// Starts a Rerun viewer to visualize the contents of a given array of messages.
/// The method will return when the viewer is closed.
///
/// ⚠️  This function must be called from the main thread since some platforms require that
/// their UI runs on the main thread! ⚠️
pub fn show(main_thread_token: crate::MainThreadToken, msgs: Vec<LogMsg>) -> eframe::Result {
    let log_source = re_log_channel::LogSource::Sdk; // Whatever. Nobody is really using this functions anyhow.

    if msgs.is_empty() {
        re_log::debug!("Empty array of msgs - call to show() ignored");
        return Ok(());
    }

    let store_source = re_log_types::StoreSource::RustSdk {
        rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
        llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
    };

    let runtime =
        tokio::runtime::Runtime::new().map_err(|err| eframe::Error::AppCreation(Box::new(err)))?;

    let startup_options = re_viewer::StartupOptions::default();

    let (tx, rx) = re_log_channel::log_channel(log_source);
    for log_msg in msgs {
        tx.send(log_msg.into()).ok();
    }

    let force_wgpu_backend = startup_options.force_wgpu_backend.clone();
    re_viewer::run_native_app(
        main_thread_token,
        Box::new(move |cc| {
            let mut app = re_viewer::App::new(
                main_thread_token,
                re_build_info::build_info!(),
                re_viewer::AppEnvironment::from_store_source(&store_source),
                startup_options,
                cc,
                None,
                re_viewer::AsyncRuntimeHandle::new_native(runtime.handle().clone()),
            );
            app.add_log_receiver(rx);
            Ok(Box::new(app))
        }),
        force_wgpu_backend.as_deref(),
    )
}
