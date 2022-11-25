use re_log_types::LogMsg;

use crate::DesignTokens;

type AppCreator =
    Box<dyn FnOnce(&eframe::CreationContext<'_>, DesignTokens) -> Box<dyn eframe::App>>;

pub fn run_native_app(app_creator: AppCreator) {
    let native_options = eframe::NativeOptions {
        depth_buffer: 0,
        multisampling: 0, // the 3D views do their own MSAA

        renderer: eframe::Renderer::Wgpu,

        initial_window_size: Some([1600.0, 1200.0].into()),
        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,

        #[cfg(target_os = "macos")]
        fullsize_content: crate::FULLSIZE_CONTENT,

        wgpu_options: crate::wgpu_options(),

        ..Default::default()
    };

    eframe::run_native(
        "Rerun Viewer",
        native_options,
        Box::new(move |cc| {
            let design_tokens = crate::customize_eframe(cc);
            app_creator(cc, design_tokens)
        }),
    );
}

pub fn run_native_viewer_with_messages(log_messages: Vec<LogMsg>) {
    let (tx, rx) = re_smart_channel::smart_channel(re_smart_channel::Source::File);
    for log_msg in log_messages {
        tx.send(log_msg).ok();
    }
    run_native_app(Box::new(move |cc, design_tokens| {
        Box::new(crate::App::from_receiver(
            &cc.egui_ctx,
            design_tokens,
            cc.storage,
            rx,
        ))
    }));
}
