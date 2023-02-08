use re_log_types::LogMsg;

#[cfg(feature = "re_viewer")]
pub fn show(log_messages: Vec<LogMsg>) -> re_viewer::external::eframe::Result<()> {
    let startup_options = re_viewer::StartupOptions::default();
    re_viewer::run_native_viewer_with_messages(startup_options, log_messages)
}
