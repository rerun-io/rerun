use egui::{Color32, FontId};

/// The longest end of `text` that fits in `max_width` with a leading `…`, when laid out in `font_id`.
///
/// Returns `text` unchanged when it fits as is. `text` is expected to be a single line.
pub fn shorten_from_start(ui: &egui::Ui, text: &str, font_id: &FontId, max_width: f32) -> String {
    let galley = ui.fonts_mut(|fonts| {
        fonts.layout_no_wrap(format!("…{text}"), font_id.clone(), Color32::PLACEHOLDER)
    });

    // There is one glyph per `char`. The first glyph is the `…`, the rest are the chars of `text`.
    let Some(row) = galley.rows.first() else {
        return text.to_owned();
    };
    let [_, text_glyphs @ ..] = row.glyphs.as_slice() else {
        return text.to_owned();
    };
    let Some(ellipsis_width) = text_glyphs.first().map(|glyph| glyph.pos.x) else {
        return text.to_owned();
    };
    let row_width = row.size.x;
    if row_width - ellipsis_width <= max_width {
        return text.to_owned();
    }

    let first_fitting =
        text_glyphs.partition_point(|glyph| ellipsis_width + row_width - glyph.pos.x > max_width);

    text.char_indices().nth(first_fitting).map_or_else(
        || "…".to_owned(),
        |(index, _)| format!("…{}", &text[index..]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Short paths stay complete, while long paths keep their ending and fit the width.
    /// Shortening keeps Unicode characters intact.
    #[test]
    fn shortens_path_from_start() {
        egui::__run_test_ui(|ui| {
            let font_id = FontId::monospace(12.0);
            let width = |text: &str| {
                ui.fonts_mut(|fonts| {
                    fonts.layout_no_wrap(text.to_owned(), font_id.clone(), Color32::PLACEHOLDER)
                })
                .size()
                .x
            };
            let char_width = width("a");

            assert_eq!(
                shorten_from_start(ui, "/camera", &font_id, 10.0 * char_width),
                "/camera"
            );
            assert_eq!(
                shorten_from_start(ui, "/world/camera", &font_id, width("…/camera") + 0.5),
                "…/camera"
            );

            let unicode = "/世界/camera";
            for max_width in [width("…/camera"), width("…界/camera"), width(unicode) - 0.5] {
                let shortened = shorten_from_start(ui, unicode, &font_id, max_width);
                assert!(shortened.starts_with('…'));
                assert!(unicode.ends_with(&shortened['…'.len_utf8()..]));
                assert!(width(&shortened) <= max_width + 0.5);
            }

            assert_eq!(shorten_from_start(ui, "/camera", &font_id, 0.0), "…");
            assert_eq!(shorten_from_start(ui, "", &font_id, 0.0), "");
        });
    }
}
