//! Rerun Data Ui
//!
//! This crate provides ui elements for Rerun component data for the Rerun Viewer.

use re_log_types::EntityPath;
use re_types::{ComponentDescriptor, RowId};
use re_viewer_context::{UiLayout, ViewerContext};

mod annotation_context;
mod app_id;
mod blob;
mod component;
mod component_path;
mod component_type;
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
pub use image::image_preview_ui;
use re_types_core::ArchetypeName;
use re_types_core::reflection::Reflection;

pub type ArchetypeComponentMap =
    std::collections::BTreeMap<Option<ArchetypeName>, Vec<ComponentDescriptor>>;

/// Components grouped by archetype.
pub fn sorted_component_list_by_archetype_for_ui<'a>(
    reflection: &Reflection,
    iter: impl IntoIterator<Item = &'a ComponentDescriptor> + 'a,
) -> ArchetypeComponentMap {
    let mut map = iter
        .into_iter()
        .filter(|d| !d.is_indicator_component())
        .fold(ArchetypeComponentMap::default(), |mut acc, descriptor| {
            acc.entry(descriptor.archetype)
                .or_default()
                .push(descriptor.clone());
            acc
        });

    for (archetype, components) in &mut map {
        if let Some(reflection) = archetype
            .as_ref()
            .and_then(|a| reflection.archetypes.get(a))
        {
            // Sort components by their importance
            components.sort_by_key(|c| {
                reflection
                    .fields
                    .iter()
                    .position(|field| field.name == c.archetype_field_name())
                    .unwrap_or(usize::MAX)
            });
        } else {
            // As a fallback, sort by the short name, as that is what is shown in the UI.
            components.sort_by_key(|c| c.display_name().to_owned());
        }
    }

    map
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
        component_descriptor: &ComponentDescriptor,
        row_id: Option<RowId>,
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
        _component_descriptor: &ComponentDescriptor,
        _row_id: Option<RowId>,
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
