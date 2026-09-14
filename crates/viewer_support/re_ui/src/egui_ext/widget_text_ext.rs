use egui::{RichText, WidgetText};

/// Concatenate several differently-styled [`RichText`]s into a single [`WidgetText`].
pub fn concat_rich_text(
    style: &egui::Style,
    parts: impl IntoIterator<Item = RichText>,
) -> WidgetText {
    let mut job = egui::text::LayoutJob::default();
    for part in parts {
        part.append_to(
            &mut job,
            style,
            egui::FontSelection::Default,
            egui::Align::Center,
        );
    }
    job.into()
}
