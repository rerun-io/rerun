use re_entity_db::{EntityDb, LatencySnapshot};
use re_sorbet::TimestampLocation;
use re_ui::UiExt as _;

/// Latency above this is not believable, and is hidden from the top bar.
pub const MAX_PLAUSIBLE_LATENCY_SEC: f32 = 10.0 * 60.0;

const E2E_HOVER_TEXT: &str = "End-to-end latency from when the data was logged by the SDK to when it is shown in the viewer.\n\
    This includes time for encoding, network latency, and decoding.\n\
    It is also affected by the frame rate of the viewer.\n\
    This latency is inaccurate if the logging was done on a different machine, since it is clock-based.";

pub fn latency_tab_ui(ui: &mut egui::Ui, recording: Option<&EntityDb>) {
    // Keep the tab from reporting a tiny content height when it only has a single label,
    // without imposing a fixed minimum size on the resizable dev panel.
    ui.set_min_height(ui.available_height());

    let Some(recording) = recording else {
        ui.label("No active recording.");
        return;
    };

    match &recording.data_source {
        Some(source) if source.may_be_live() => {}
        Some(source) => {
            ui.label(format!(
                "{source} replays data that was logged earlier, so there is no latency to measure."
            ));
            return;
        }
        None => {
            ui.label("This recording has no data source, so there is no latency to measure.");
            return;
        }
    }

    let latency = recording.ingestion_stats().latency_snapshot();

    let Some(e2e) = latency.e2e() else {
        ui.label("No data has arrived in the last second, so there is no latency to report.");
        return;
    };

    let LatencySnapshot { secs_since_log } = latency;

    ui.horizontal(|ui| {
        ui.strong("End-to-end latency:");
        latency_label(ui, e2e);
    })
    .response
    .on_hover_text(E2E_HOVER_TEXT);

    if MAX_PLAUSIBLE_LATENCY_SEC < e2e {
        ui.warning_label(
            "This latency is too high to be believable: either the sender's clock is off, or this is not live data.",
        );
    }

    ui.label(
        "Time spent in each step of the pipeline, starting at the `log` call in the SDK. \
        This is a rolling average over the last second of incoming data.",
    );

    ui.separator();

    egui::Grid::new("latency steps")
        .num_columns(3)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Step").underline());
            ui.label(egui::RichText::new("Duration").underline())
                .on_hover_text("Time since the previous step");
            ui.label(egui::RichText::new("Since log call").underline());
            ui.end_row();

            let mut previous_sec = 0.0;
            for (location, since_log_sec) in secs_since_log {
                if location == TimestampLocation::Log {
                    // The `log` call is the zero-point of the measurement, so it has nothing to show.
                    continue;
                }

                ui.label(location.to_string());
                latency_label(ui, since_log_sec - previous_sec);
                latency_label(ui, since_log_sec);
                ui.end_row();

                previous_sec = since_log_sec;
            }
        });
}

fn latency_label(ui: &mut egui::Ui, latency_sec: f32) -> egui::Response {
    ui.label(latency_text(ui.visuals(), latency_sec))
}

pub fn latency_text(visuals: &egui::Visuals, latency_sec: f32) -> egui::RichText {
    if latency_sec < 0.001 {
        egui::RichText::new(format!("{:.0} µs", 1e6 * latency_sec))
    } else if latency_sec < 1.0 {
        egui::RichText::new(format!("{:.0} ms", 1e3 * latency_sec))
    } else {
        egui::RichText::new(format!("{latency_sec:.1} s")).color(visuals.warn_fg_color)
    }
}
