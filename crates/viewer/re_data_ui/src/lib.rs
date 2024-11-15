//! Rerun Data Ui
//!
//! This crate provides ui elements for Rerun component data for the Rerun Viewer.

use re_log_types::EntityPath;
use re_types::ComponentName;
use re_viewer_context::{UiLayout, ViewerContext};

mod annotation_context;
mod app_id;
mod blob;
mod component;
mod component_name;
mod component_path;
mod component_ui_registry;
mod data_source;
mod entity_db;
mod entity_path;
mod image;
mod instance_path;
mod store_id;
mod tensor;
mod video;

pub mod item_ui;

pub use crate::tensor::tensor_summary_ui_grid_contents;
pub use component::ComponentPathLatestAtResults;
pub use component_ui_registry::{add_to_registry, register_component_uis};

/// Sort components for display in the UI.
pub fn sorted_component_list_for_ui<'a>(
    iter: impl IntoIterator<Item = &'a ComponentName> + 'a,
) -> Vec<ComponentName> {
    let mut components: Vec<ComponentName> = iter.into_iter().copied().collect();

    // Put indicator components first.
    // We then sort by the short name, as that is what is shown in the UI.
    components.sort_by_key(|c| (!c.is_indicator_component(), c.short_name()));

    components
}

/// Types implementing [`DataUi`] can display themselves in an [`egui::Ui`].
pub trait DataUi {
    /// If you need to lookup something in the chunk store, use the given query to do so.
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    );

    /// Called [`Self::data_ui`] using the default query and recording.
    fn data_ui_recording(&self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui, ui_layout: UiLayout) {
        self.data_ui(ctx, ui, ui_layout, &ctx.current_query(), ctx.recording());
    }
}

/// Similar to [`DataUi`], but for data that is related to an entity (e.g. a component).
///
/// This is given the context of the entity it is part of so it can do queries.
pub trait EntityDataUi {
    /// If you need to lookup something in the chunk store, use the given query to do so.
    #[allow(clippy::too_many_arguments)]
    fn entity_data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        entity_path: &EntityPath,
        row_id: Option<re_chunk_store::RowId>,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    );
}

impl<T> EntityDataUi for T
where
    T: DataUi,
{
    fn entity_data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        entity_path: &EntityPath,
        _row_id: Option<re_chunk_store::RowId>,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        // This ensures that UI state is maintained per entity. For example, the collapsed state for
        // `AnnotationContext` component is not saved by all instances of the component.
        ui.push_id(entity_path.hash(), |ui| {
            self.data_ui(ctx, ui, ui_layout, query, db);
        });
    }
}

// ---------------------------------------------------------------------------

pub fn annotations(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_entity_db::EntityPath,
) -> std::sync::Arc<re_viewer_context::Annotations> {
    re_tracing::profile_function!();
    let mut annotation_map = re_viewer_context::AnnotationMap::default();
    annotation_map.load(ctx, query, std::iter::once(entity_path));
    annotation_map.find(entity_path)
}
