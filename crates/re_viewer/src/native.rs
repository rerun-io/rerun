use std::sync::mpsc::Receiver;

use re_log_types::LogMsg;

pub fn run_native_app(app_creator: eframe::AppCreator) {
    let native_options = eframe::NativeOptions {
        depth_buffer: 24,
        multisampling: 8,
        initial_window_size: Some([1600.0, 1200.0].into()),
        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,
        ..Default::default()
    };

    eframe::run_native(
        "rerun viewer",
        native_options,
        Box::new(move |cc| {
            crate::customize_egui(&cc.egui_ctx);
            app_creator(cc)
        }),
    );
}

pub fn run_native_viewer_with_rx(rx: Receiver<LogMsg>) {
    run_native_app(Box::new(move |cc| {
        let rx = wake_up_ui_thread_on_each_msg(rx, cc.egui_ctx.clone());
        Box::new(crate::App::from_receiver(&cc.egui_ctx, cc.storage, rx))
    }));
}

fn wake_up_ui_thread_on_each_msg<T: Send + 'static>(
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
            tracing::debug!("Shutting down ui_waker thread");
        })
        .unwrap();
    new_rx
}
