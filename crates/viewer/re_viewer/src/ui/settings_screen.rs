use std::str::FromStr as _;

use egui::{NumExt as _, Ui};
use re_data_source::StreamMode;
use re_log_types::{Timestamp, TimestampFormat};
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::{DesignTokens, UiExt as _};
use re_viewer_context::{AppOptions, ExperimentalAppOptions, VideoOptions};

use crate::StartupOptions;

pub fn settings_screen_ui(
    ui: &mut egui::Ui,
    app_options: &mut AppOptions,
    startup_options: &mut StartupOptions,
    keep_open: &mut bool,
) {
    egui::Frame {
        inner_margin: egui::Margin::same(5),
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
                settings_screen_ui_impl(ui, app_options, startup_options, keep_open);
            });

        if ui.input_mut(|ui| ui.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            *keep_open = false;
        }
    });
}

fn settings_screen_ui_impl(
    ui: &mut egui::Ui,
    app_options: &mut AppOptions,
    startup_options: &mut StartupOptions,
    keep_open: &mut bool,
) {
    //
    // Title
    //

    ui.add_space(40.0);

    ui.horizontal(|ui| {
        ui.add(egui::Label::new(
            egui::RichText::new("Settings")
                .strong()
                .line_height(Some(32.0))
                .text_style(DesignTokens::welcome_screen_h2()),
        ));

        ui.allocate_ui_with_layout(
            egui::Vec2::X * ui.available_width(),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                if ui
                    .small_icon_button(&re_ui::icons::CLOSE, "Close")
                    .clicked()
                {
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

    ui.horizontal(|ui| {
        ui.label("Theme");
        egui::global_theme_preference_buttons(ui);
    });

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label("Memory budget");
        memory_budget_section_ui(ui, startup_options);
        ui.help_button(|ui| {
            ui.label("When this limit is reached we start purging data from RAM");
        });
    });

    let AppOptions {
        experimental,
        warn_e2e_latency: _, // not yet exposed
        show_metrics,
        show_notification_toasts,
        include_rerun_examples_button_in_recordings_panel,
        show_picking_debug_overlay: _, // not yet exposed
        inspect_blueprint_timeline: _, // not yet exposed
        blueprint_gc: _,               // not yet exposed
        timestamp_format,
        video,
        mapbox_access_token,

        #[cfg(not(target_arch = "wasm32"))]
            cache_directory: _, // not yet exposed
    } = app_options;

    ui.add_space(8.0);

    ui.re_checkbox(
        include_rerun_examples_button_in_recordings_panel,
        "Show 'Rerun examples' button",
    );

    ui.re_checkbox(show_metrics, "Show performance metrics")
        .on_hover_text("Show metrics for milliseconds/frame and RAM usage in the top bar");

    ui.re_checkbox(show_notification_toasts, "Show notification toasts")
        .on_hover_text("Show toasts for log messages and other notifications");

    separator_with_some_space(ui);
    ui.collapsing_header("Timestamp format", false, |ui| {
        time_format_section_ui(ui, timestamp_format);
    });

    separator_with_some_space(ui);
    ui.strong("Map view");
    map_view_section_ui(ui, mapbox_access_token);

    separator_with_some_space(ui);
    ui.strong("Video");
    video_section_ui(ui, video);

    separator_with_some_space(ui);
    ui.strong("Experimental");
    experimental_section_ui(ui, experimental);
}

fn experimental_section_ui(ui: &mut Ui, experimental: &mut ExperimentalAppOptions) {
    let ExperimentalAppOptions {
        stream_mode,
        component_mapping,
    } = experimental;

    let mut larger_than_ram = *stream_mode == StreamMode::OnDemand;
    ui.re_checkbox(&mut larger_than_ram, "Larger-than-RAM streaming");
    *stream_mode = if larger_than_ram {
        StreamMode::OnDemand
    } else {
        StreamMode::FullLoad
    };

    if larger_than_ram {
        ui.warning_label("This is an experimental feature that is not yet fully supported.");
    }

    ui.re_checkbox(
            component_mapping,
            "Component mapping ui"
        )
        .on_hover_ui(|ui| {
            ui.markdown_ui(
                "Enables an experimental UI for mapping components to different visualizations in the visualizer section of the selection view.
WARNING: Viewer restart is needed to apply some of the remapping abilities to existing views.",
            );
        });
}

fn memory_budget_section_ui(ui: &mut Ui, startup_options: &mut StartupOptions) {
    const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;
    const UPPER_LIMIT_BYTES: u64 = 1_000 * BYTES_PER_GIB;

    let mut bytes = startup_options.memory_limit.max_bytes.unwrap_or(u64::MAX);

    let speed = (0.02 * bytes as f32).clamp(0.01 * BYTES_PER_GIB as f32, BYTES_PER_GIB as f32);

    ui.add(
        egui::DragValue::new(&mut bytes)
            .custom_formatter(|bytes, _| {
                if bytes < UPPER_LIMIT_BYTES as f64 {
                    re_format::format_bytes(bytes)
                } else {
                    "unlimited".to_owned()
                }
            })
            .custom_parser(|s| {
                let s = s.trim();
                if s.chars().all(|c| c.is_numeric()) {
                    // Assume GB
                    Some(BYTES_PER_GIB as f64 * f64::from_str(s).ok()?)
                } else {
                    Some(re_format::parse_bytes(s)? as f64)
                }
            })
            .update_while_editing(false)
            .range(0..=UPPER_LIMIT_BYTES)
            .speed(speed),
    );

    if bytes < UPPER_LIMIT_BYTES {
        startup_options.memory_limit.max_bytes = Some(bytes);
    } else {
        startup_options.memory_limit.max_bytes = None;
    }
}

fn time_format_section_ui(ui: &mut Ui, timestamp_format: &mut TimestampFormat) {
    fn timestamp_example_ui(
        ui: &mut egui::Ui,
        timestamp: Timestamp,
        timestamp_format: TimestampFormat,
    ) {
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().icon_width + ui.spacing().icon_spacing);
            egui::Frame::new()
                .fill(ui.visuals().text_edit_bg_color())
                .corner_radius(2.0)
                .inner_margin(egui::Margin::symmetric(4, 2))
                .show(ui, |ui| {
                    ui.label(
                        SyntaxHighlightedBuilder::primitive(&timestamp.format(timestamp_format))
                            .into_widget_text(ui.style()),
                    );
                });
        });
    }

    let timestamp = re_log_types::Timestamp::from(
        jiff::Timestamp::from_str("2023-02-14 21:47:18Z").expect("the timestamp is valid"),
    );

    ui.re_radio_value(timestamp_format, TimestampFormat::utc(), "UTC");
    timestamp_example_ui(ui, timestamp, TimestampFormat::utc());
    ui.re_radio_value(
        timestamp_format,
        TimestampFormat::local_timezone(),
        "Local (show time zone)",
    );
    timestamp_example_ui(ui, timestamp, TimestampFormat::local_timezone());
    ui.re_radio_value(
        timestamp_format,
        TimestampFormat::local_timezone_implicit(),
        "Local (hide time zone)",
    );
    timestamp_example_ui(ui, timestamp, TimestampFormat::local_timezone_implicit());
    ui.horizontal(|ui| {
        ui.add_space(ui.spacing().icon_width + ui.spacing().icon_spacing);
        ui.label("Note: timestamps without time zone are ambiguous when copied elsewhere.");
    });

    ui.re_radio_value(
        timestamp_format,
        TimestampFormat::unix_epoch(),
        "Seconds since Unix epoch",
    );
    timestamp_example_ui(ui, timestamp, TimestampFormat::unix_epoch());
}

fn map_view_section_ui(ui: &mut Ui, mapbox_access_token: &mut String) {
    ui.horizontal(|ui| {
        // TODO(ab): needed for alignment, we should use egui flex instead
        ui.set_height(19.0);

        ui.label("Mapbox access token:").on_hover_ui(|ui| {
            ui.markdown_ui(
                "This token is used to enable Mapbox-based map view backgrounds.\n\n\
                Note that the token will be saved in clear text in the configuration file. \
                The token can also be set using the `RERUN_MAPBOX_ACCESS_TOKEN` environment \
                variable.",
            );
        });

        ui.add(egui::TextEdit::singleline(mapbox_access_token).password(true));
    });
}

fn video_section_ui(ui: &mut Ui, options: &mut VideoOptions) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        ui.re_checkbox(
            &mut options.override_ffmpeg_path,
            "Override the FFmpeg binary path",
        )
        .on_hover_ui(|ui| {
            ui.markdown_ui(
                "By default, the viewer tries to automatically find a suitable FFmpeg binary in \
                the system's `PATH`. Enabling this option allows you to specify a custom path to \
                the FFmpeg binary.",
            );
        });

        ui.add_enabled_ui(options.override_ffmpeg_path, |ui| {
            ui.horizontal(|ui| {
                // TODO(ab): needed for alignment, we should use egui flex instead
                ui.set_height(19.0);

                ui.label("Path:");

                ui.add(egui::TextEdit::singleline(&mut options.ffmpeg_path));
            });
        });

        ffmpeg_path_status_ui(ui, options);
    }

    // This affects only the web target, so we don't need to show it on native.
    #[cfg(target_arch = "wasm32")]
    {
        use re_video::DecodeHardwareAcceleration;

        let hardware_acceleration = &mut options.hw_acceleration;
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
fn ffmpeg_path_status_ui(ui: &mut Ui, options: &VideoOptions) {
    use std::task::Poll;

    use re_video::{FFmpegVersion, FFmpegVersionParseError};

    let path = options
        .override_ffmpeg_path
        .then(|| std::path::Path::new(&options.ffmpeg_path));

    match FFmpegVersion::for_executable_poll(path) {
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

        Poll::Ready(Err(FFmpegVersionParseError::FFmpegNotFound(_path))) => {
            ui.error_label("The specified FFmpeg binary path does not exist or is not a file.");
        }

        Poll::Ready(Err(err)) => {
            ui.error_label(err.to_string());
        }
    }
}

fn separator_with_some_space(ui: &mut egui::Ui) {
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}
