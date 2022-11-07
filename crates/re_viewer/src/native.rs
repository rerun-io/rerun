use std::sync::mpsc::Receiver;

use re_log_types::LogMsg;

use crate::DesignTokens;

#[cfg(not(any(feature = "glow", feature = "wgpu")))]
compile_error!("You must enable either the 'glow' or 'wgpu' feature of re_viewer.");

type AppCreator =
    Box<dyn FnOnce(&eframe::CreationContext<'_>, DesignTokens) -> Box<dyn eframe::App>>;

pub fn run_native_app(app_creator: AppCreator) {
    let native_options = eframe::NativeOptions {
        #[cfg(not(feature = "wgpu"))]
        depth_buffer: 24,
        #[cfg(feature = "wgpu")]
        depth_buffer: 0,

        #[cfg(not(feature = "wgpu"))]
        multisampling: 8,

        #[cfg(feature = "glow")]
        renderer: eframe::Renderer::Glow,
        #[cfg(not(feature = "glow"))]
        renderer: eframe::Renderer::Wgpu,

        initial_window_size: Some([1600.0, 1200.0].into()),
        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,

        #[cfg(target_os = "macos")]
        fullsize_content: crate::FULLSIZE_CONTENT,

        #[cfg(feature = "wgpu")]
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

pub fn run_native_viewer_with_rx(rx: Receiver<LogMsg>) {
    run_native_app(Box::new(move |cc, design_tokens| {
        let rx = wake_up_ui_thread_on_each_msg(rx, cc.egui_ctx.clone());
        Box::new(crate::App::from_receiver(
            &cc.egui_ctx,
            design_tokens,
            cc.storage,
            rx,
        ))
    }));
}

pub fn wake_up_ui_thread_on_each_msg<T: Send + 'static>(
    rx: Receiver<T>,
    ctx: egui::Context,
) -> Receiver<T> {
    let (tx, new_rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("ui_waker".to_owned())
        .spawn(move || {
            while let Ok(msg) = rx.recv() {
                if tx.send(msg).is_ok() {
                    ctx.request_repaint();
                } else {
                    break;
                }
            }
            re_log::debug!("Shutting down ui_waker thread");
        })
        .unwrap();
    new_rx
}
