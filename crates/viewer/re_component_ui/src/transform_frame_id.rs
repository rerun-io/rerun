use re_sdk_types::components::TransformFrameId;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::text_edit::autocomplete_text_edit;
use re_viewer_context::{MaybeMutRef, StoreViewContext, UiLayout};

/// Shows a potentially editable `frame_id`.
/// If the `frame_id` is being edited, a list of matching frame names is shown as suggestions.
///
/// Note: implicit, entity-path-derived frame IDs are not suggested.
pub fn edit_or_view_transform_frame_id(
    ctx: &StoreViewContext<'_>,
    ui: &mut egui::Ui,
    frame_id: &mut MaybeMutRef<'_, TransformFrameId>,
) -> egui::Response {
    match frame_id {
        MaybeMutRef::Ref(frame_id) => UiLayout::List.data_label(
            ui,
            SyntaxHighlightedBuilder::new().with_string_value(frame_id.as_str()),
        ),
        MaybeMutRef::MutRef(frame_id) => {
            let suggestions = if let Some(store_ctx) = ctx.active_store_context {
                let caches = store_ctx.caches;
                let frame_id_registry =
                    caches.memoizer(|c: &mut re_viewer_context::TransformDatabaseStoreCache| {
                        c.frame_id_registry(store_ctx.recording)
                    });

                frame_id_registry
                    .iter_frame_ids()
                    .filter(|(_, id)| !id.is_entity_path_derived())
                    .map(|(_, id)| id.to_string())
                    .collect::<Vec<String>>()
            } else {
                Vec::new()
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
