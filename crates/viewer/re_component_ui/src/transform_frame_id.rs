use re_sdk_types::{TransformFrameIdHash, components::TransformFrameId};
use re_ui::UiExt as _;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn edit_or_view_transform_frame_id(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    frame_id: &mut MaybeMutRef<'_, TransformFrameId>,
    hint_text: Option<&str>,
) -> egui::Response {
    match frame_id {
        MaybeMutRef::Ref(frame_id) => ui.label(frame_id.as_str()),
        MaybeMutRef::MutRef(frame_id) => edit_transform_frame_id(ctx, ui, frame_id, hint_text),
    }
}

fn edit_transform_frame_id(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    frame_id: &mut TransformFrameId,
    hint_text: Option<impl Into<egui::WidgetText>>,
) -> egui::Response {
    let (mut suggestions, mut response) = {
        // In a scope to not hold the lock for longer than needed.
        let caches = ctx.store_context.caches;
        let transform_cache =
            caches.entry(|c: &mut re_viewer_context::TransformDatabaseStoreCache| {
                c.read_lock_transform_cache(ctx.recording())
            });

        let frame_exists = transform_cache
            .frame_id_registry()
            .lookup_frame_id(TransformFrameIdHash::from_str(&*frame_id))
            .is_some();

        let mut tmp_string = frame_id.as_str().to_owned();

        let mut text_edit = egui::TextEdit::singleline(&mut tmp_string);
        if let Some(hint) = hint_text {
            text_edit = text_edit.hint_text(hint);
        }
        if !frame_exists {
            text_edit = text_edit.text_color(ui.tokens().error_fg_color);
        }
        let response = ui.add(text_edit);

        let suggestions = transform_cache
            .frame_id_registry()
            .iter_frame_ids()
            // Only show named frames.
            .filter(|(_, id)| !id.is_entity_path_derived())
            .filter_map(|(_, id)| id.strip_prefix(tmp_string.as_str()))
            .filter(|rest| !rest.is_empty())
            .map(|rest| rest.to_owned())
            .collect::<Vec<_>>();

        if response.changed() {
            *frame_id = TransformFrameId::new(tmp_string.as_str());
        }

        (suggestions, response)
    };

    suggestions.sort_unstable();

    let suggestions_open =
        (response.has_focus() || response.lost_focus()) && !suggestions.is_empty();

    let width = response.rect.width();

    let mut changed = false;
    let suggestions_ui = |ui: &mut egui::Ui| {
        for rest in suggestions {
            let mut layout_job = egui::text::LayoutJob::default();
            layout_job.append(
                &*frame_id,
                0.0,
                egui::TextFormat::simple(
                    ui.style().text_styles[&egui::TextStyle::Body].clone(),
                    ui.tokens().text_default,
                ),
            );
            layout_job.append(
                &rest,
                0.0,
                egui::TextFormat::simple(
                    ui.style().text_styles[&egui::TextStyle::Body].clone(),
                    ui.tokens().text_subdued,
                ),
            );

            if ui
                .add(egui::Button::new(layout_job).min_size(egui::vec2(width, 0.0)))
                .clicked()
            {
                changed = true;
                *frame_id = TransformFrameId::new(&format!("{frame_id}{rest}"));
            }
        }
    };

    egui::Popup::from_response(&response)
        .style(re_ui::menu::menu_style())
        .open(suggestions_open)
        .show(|ui: &mut egui::Ui| {
            ui.set_width(width);

            egui::ScrollArea::vertical()
                .min_scrolled_height(350.0)
                .max_height(350.0)
                .show(ui, suggestions_ui);
        });

    if changed {
        response.mark_changed();
    }

    response
}
