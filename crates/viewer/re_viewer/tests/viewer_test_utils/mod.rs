use egui_kittest::Harness;
use re_build_info::build_info;
use re_viewer::{
    App, AsyncRuntimeHandle, MainThreadToken, StartupOptions, customize_eframe_and_setup_renderer,
};

pub fn viewer_harness() -> Harness<'static, re_viewer::App> {
    Harness::builder()
        .wgpu()
        .with_size(egui::vec2(1500., 1000.))
        .build_eframe(|cc| {
            cc.egui_ctx.set_os(egui::os::OperatingSystem::Nix);
            customize_eframe_and_setup_renderer(cc).expect("Failed to customize eframe");
            App::new(
                MainThreadToken::i_promise_i_am_on_the_main_thread(),
                build_info!(),
                re_viewer::AppEnvironment::Test,
                StartupOptions::default(),
                cc,
                None,
                AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().expect("broooken"),
            )
        })
}
