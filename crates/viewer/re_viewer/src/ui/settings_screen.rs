use egui::{NumExt as _, Ui};

use re_log_types::TimeZone;
use re_ui::UiExt as _;
use re_viewer_context::AppOptions;

pub fn settings_screen_ui(ui: &mut egui::Ui, app_options: &mut AppOptions, keep_open: &mut bool) {
    egui::Frame {
        inner_margin: egui::Margin::same(5.0),
        ..Default::default()
    }
    .show(ui, |ui| {
        const MAX_WIDTH: f32 = 600.0;
        const MIN_WIDTH: f32 = 300.0;

        let centering_margin = ((ui.available_width() - MAX_WIDTH) / 2.0).at_least(0.0);
        let max_rect = ui.max_rect().expand2(-centering_margin * egui::Vec2::X);
        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(max_rect));

        egui::ScrollArea::both()
            .auto_shrink(false)
            .show(&mut child_ui, |ui| {
                ui.set_min_width(MIN_WIDTH);
                settings_screen_ui_impl(ui, app_options, keep_open);
            });

        if ui.input_mut(|ui| ui.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            *keep_open = false;
        }
    });
}

fn settings_screen_ui_impl(ui: &mut egui::Ui, app_options: &mut AppOptions, keep_open: &mut bool) {
    //
    // Title
    //

    ui.add_space(40.0);

    ui.horizontal(|ui| {
        ui.add(egui::Label::new(
            egui::RichText::new("Settings")
                .strong()
                .line_height(Some(32.0))
                .text_style(re_ui::DesignTokens::welcome_screen_h2()),
        ));

        ui.allocate_ui_with_layout(
            egui::Vec2::X * ui.available_width(),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                if ui.small_icon_button(&re_ui::icons::CLOSE).clicked() {
                    *keep_open = false;
                }
            },
        )
    });

    //
    // General
    //

    separator_with_some_space(ui);

    ui.strong("General");

    ui.re_checkbox(
        &mut app_options.include_welcome_screen_button_in_recordings_panel,
        "Show 'Welcome screen' button",
    );

    ui.re_checkbox(&mut app_options.show_metrics, "Show performance metrics")
        .on_hover_text("Show metrics for milliseconds/frame and RAM usage in the top bar");

    //
    // Timezone
    //

    separator_with_some_space(ui);

    ui.strong("Timezone");
    ui.re_radio_value(&mut app_options.time_zone, TimeZone::Utc, "UTC")
        .on_hover_text("Display timestamps in UTC");
    ui.re_radio_value(&mut app_options.time_zone, TimeZone::Local, "Local")
        .on_hover_text("Display timestamps in the local timezone");
    ui.re_radio_value(
        &mut app_options.time_zone,
        TimeZone::UnixEpoch,
        "Unix epoch",
    )
    .on_hover_text("Display timestamps in seconds since unix epoch");

    //
    // Map view
    //

    separator_with_some_space(ui);

    ui.strong("Map view");

    ui.horizontal(|ui| {
        // TODO(ab): needed for alignment, we should use egui flex instead
        ui.set_height(19.0);

        ui.label("Mapbox access token:").on_hover_ui(|ui| {
            ui.markdown_ui(
                "This token is used toe enable Mapbox-based map view backgrounds.\n\n\
                Note that the token will be saved in clear text in the configuration file. \
                The token can also be set using the `RERUN_MAPBOX_ACCESS_TOKEN` environment \
                variable.",
            );
        });

        ui.add(egui::TextEdit::singleline(&mut app_options.mapbox_access_token).password(true));
    });

    //
    // Video
    //

    separator_with_some_space(ui);
    ui.strong("Video");
    video_section_ui(ui, app_options);

    //
    // Experimental features
    //

    // Currently, the wasm target does not have any experimental features. If/when that changes,
    // move the conditional compilation flag to the respective checkbox code.
    #[cfg(not(target_arch = "wasm32"))]
    // Currently there are no experimental features
    if false {
        separator_with_some_space(ui);
        ui.strong("Experimental features");
    }
}

fn video_section_ui(ui: &mut Ui, app_options: &mut AppOptions) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        ui.re_checkbox(
            &mut app_options.video_decoder_override_ffmpeg_path,
            "Override the FFmpeg binary path",
        )
        .on_hover_ui(|ui| {
            ui.markdown_ui(
                "By default, the viewer tries to automatically find a suitable FFmpeg binary in \
                the system's `PATH`. Enabling this option allows you to specify a custom path to \
                the FFmpeg binary.",
            );
        });

        ui.add_enabled_ui(app_options.video_decoder_override_ffmpeg_path, |ui| {
            ui.horizontal(|ui| {
                // TODO(ab): needed for alignment, we should use egui flex instead
                ui.set_height(19.0);

                ui.label("Path:");

                ui.add(egui::TextEdit::singleline(
                    &mut app_options.video_decoder_ffmpeg_path,
                ));
            });
        });

        ffmpeg_path_status_ui(ui, app_options);
    }

    // This affects only the web target, so we don't need to show it on native.
    #[cfg(target_arch = "wasm32")]
    {
        use re_video::decode::DecodeHardwareAcceleration;

        let hardware_acceleration = &mut app_options.video_decoder_hw_acceleration;
        ui.horizontal(|ui| {
            ui.label("Decoder:");
            egui::ComboBox::from_id_salt("video_decoder_hw_acceleration")
                .selected_text(hardware_acceleration.to_string())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        hardware_acceleration,
                        DecodeHardwareAcceleration::Auto,
                        DecodeHardwareAcceleration::Auto.to_string(),
                    );
                    ui.selectable_value(
                        hardware_acceleration,
                        DecodeHardwareAcceleration::PreferSoftware,
                        DecodeHardwareAcceleration::PreferSoftware.to_string(),
                    );
                    ui.selectable_value(
                        hardware_acceleration,
                        DecodeHardwareAcceleration::PreferHardware,
                        DecodeHardwareAcceleration::PreferHardware.to_string(),
                    );
                });
            // Note that the setting is part of the video's cache key, so, if it changes, the cache
            // entries outdate automatically.
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn ffmpeg_path_status_ui(ui: &mut Ui, app_options: &AppOptions) {
    use re_video::decode::{FFmpegVersion, FFmpegVersionParseError};
    use std::task::Poll;

    let path = app_options
        .video_decoder_override_ffmpeg_path
        .then(|| std::path::Path::new(&app_options.video_decoder_ffmpeg_path));

    if path.is_some_and(|path| !path.is_file()) {
        ui.error_label("The specified FFmpeg binary path does not exist or is not a file.");
    } else {
        let res = FFmpegVersion::for_executable_poll(path);

        match res {
            Poll::Pending => {
                ui.spinner();
            }

            Poll::Ready(Ok(version)) => {
                if version.is_compatible() {
                    ui.success_label(format!("FFmpeg found (version {version})"));
                } else {
                    ui.error_label(format!("Incompatible FFmpeg version: {version}"));
                }
            }
            Poll::Ready(Err(FFmpegVersionParseError::ParseVersion { raw_version })) => {
                // We make this one a warning instead of an error because version parsing is flaky, and
                // it might end up still working.
                ui.warning_label(format!(
                    "FFmpeg binary found but unable to parse version: {raw_version}"
                ));
            }

            Poll::Ready(Err(err)) => {
                ui.error_label(format!("Unable to check FFmpeg version: {err}"));
            }
        }
    };
}

fn separator_with_some_space(ui: &mut egui::Ui) {
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}
