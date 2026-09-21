use re_log_types::{TimeCell, TimeType, TimestampFormat};
use re_ui::HasDesignTokens as _;

/// Show a time range and its length, fitted to the text.
/// An open-ended range displays `-` for its stop and length.
pub fn time_range_tooltip_ui(
    ui: &mut egui::Ui,
    time_type: TimeType,
    start: i64,
    stop: Option<i64>,
    timestamp_format: TimestampFormat,
) -> egui::Response {
    let format_time = |time| TimeCell::new(time_type, time).format(timestamp_format);
    let length = stop.map_or_else(
        || "-".to_owned(),
        |stop| {
            let length = i64::try_from(start.abs_diff(stop)).unwrap_or(i64::MAX);
            match time_type {
                TimeType::DurationNs | TimeType::TimestampNs => {
                    re_format::DurationFormatOptions::default()
                        .with_only_seconds(false)
                        .with_min_decimals(0)
                        .format_nanos(length)
                }
                TimeType::Sequence => re_format::format_int(length),
            }
        },
    );

    // Recompute the available width on every hover, including when its content changes.
    let available_width = ui.ctx().content_rect().width() - ui.spacing().menu_margin.sum().x;
    ui.set_max_width(ui.spacing().tooltip_width.min(available_width));

    let tokens = ui.tokens();
    egui::Grid::new("time_range_tooltip")
        .num_columns(2)
        .spacing(egui::vec2(12.0, 0.0))
        .min_col_width(0.0)
        .min_row_height(re_ui::DesignTokens::list_item_height())
        .show(ui, |ui| {
            for (label, value) in [
                ("Start", format_time(start)),
                ("Stop", stop.map_or_else(|| "-".to_owned(), format_time)),
                ("Length", length),
            ] {
                ui.label(egui::RichText::new(label).color(tokens.list_item_noninteractive_text));
                ui.label(egui::RichText::new(value).color(tokens.list_item_strong_text));
                ui.end_row();
            }
        })
        .response
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A time range hover fits short values and grows when longer values replace them.
    /// Returning to short values reduces its width again.
    #[test]
    fn tooltip_fits_changing_values() {
        let ctx = egui::Context::default();
        let width_for = |stop| {
            let mut width = 0.0;
            for _ in 0..5 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800.0, 300.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        egui::Area::new(egui::Id::unique("range_hover")).show(ui.ctx(), |ui| {
                            width = time_range_tooltip_ui(
                                ui,
                                TimeType::Sequence,
                                0,
                                Some(stop),
                                TimestampFormat::default(),
                            )
                            .rect
                            .width();
                        });
                    },
                );
                output.textures_delta.clear();
            }
            width
        };
        let short_width = width_for(1);
        let long_width = width_for(i64::MAX);
        let short_width_again = width_for(1);

        assert!(short_width < 200.0, "{short_width}");
        assert!(
            long_width > short_width + 50.0,
            "{long_width} vs {short_width}"
        );
        assert!(long_width < 400.0, "{long_width}");
        assert!((short_width_again - short_width).abs() < 1.0);
    }
}
