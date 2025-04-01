use re_viewer_context::{TableStore, ViewerContext};

/// Display a dataframe table
pub(crate) fn table_ui(_ctx: &ViewerContext<'_>, ui: &mut egui::Ui, batch_store: &TableStore) {
    re_tracing::profile_function!();

    let batches = batch_store.batches();

    if batches.is_empty() {
        ui.label("No batches available");
        return;
    }

    // Queue "draw the rest of the owl meme here."
    for batch in batch_store.batches() {
        ui.label(format!("Batch: {:?}", batch));
    }
}
