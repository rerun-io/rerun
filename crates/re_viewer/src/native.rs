use re_log_types::LogMsg;

type AppCreator =
    Box<dyn FnOnce(&eframe::CreationContext<'_>, re_ui::ReUi) -> Box<dyn eframe::App>>;

pub fn run_native_app(app_creator: AppCreator) -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        initial_window_size: Some([1600.0, 1200.0].into()),
        min_window_size: Some([600.0, 240.0].into()),

        #[cfg(target_os = "macos")]
        fullsize_content: re_ui::FULLSIZE_CONTENT,

        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,

        renderer: eframe::Renderer::Wgpu,
        wgpu_options: crate::wgpu_options(),
        depth_buffer: 0,
        multisampling: 0, // the 3D views do their own MSAA

        ..Default::default()
    };

    eframe::run_native(
        "Rerun Viewer",
        native_options,
        Box::new(move |cc| {
            let re_ui = crate::customize_eframe(cc);
            app_creator(cc, re_ui)
        }),
    )
}

pub fn run_native_viewer_with_messages(
    startup_options: crate::StartupOptions,
    log_messages: Vec<LogMsg>,
) -> eframe::Result<()> {
    let (tx, rx) = re_smart_channel::smart_channel(re_smart_channel::Source::File);
    for log_msg in log_messages {
        tx.send(log_msg).ok();
    }
    run_native_app(Box::new(move |cc, re_ui| {
        Box::new(crate::App::from_receiver(
            startup_options,
            re_ui,
            cc.storage,
            rx,
        ))
    }))
}
