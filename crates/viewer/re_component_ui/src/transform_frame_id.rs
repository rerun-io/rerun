use re_sdk_types::components::TransformFrameId;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::text_edit::autocomplete_text_edit;
use re_viewer_context::{MaybeMutRef, UiLayout, ViewerContext};

/// Shows a potentially editable `frame_id`.
/// If the `frame_id` is being edited, a list of matching frame names is shown as suggestions.
///
/// Note: implicit, entity-path-derived frame IDs are not suggested.
pub fn edit_or_view_transform_frame_id(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    frame_id: &mut MaybeMutRef<'_, TransformFrameId>,
) -> egui::Response {
    match frame_id {
        MaybeMutRef::Ref(frame_id) => UiLayout::List.data_label(
            ui,
            SyntaxHighlightedBuilder::new().with_string_value(frame_id.as_str()),
        ),
        MaybeMutRef::MutRef(frame_id) => {
            let suggestions = {
                let caches = ctx.store_context.caches;
                let frame_id_registry =
                    caches.entry(|c: &mut re_viewer_context::TransformDatabaseStoreCache| {
                        c.frame_id_registry(ctx.recording())
                    });

                frame_id_registry
                    .iter_frame_ids()
                    .filter(|(_, id)| !id.is_entity_path_derived())
                    .map(|(_, id)| id.to_string())
                    .collect::<Vec<String>>()
            };

            let mut tmp_string = frame_id.as_str().to_owned();
            let response = autocomplete_text_edit(ui, &mut tmp_string, &suggestions, None::<&str>);
            if response.changed() {
                **frame_id = TransformFrameId::new(&tmp_string);
            }
            response
        }
    }
}
