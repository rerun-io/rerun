//! Rerun Data Ui
//!
//! This crate provides ui elements for Rerun component data for the Rerun Viewer.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

use re_log_types::EntityPath;
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_sdk_types::{ComponentDescriptor, RowId};
use re_ui::UiExt as _;
use re_viewer_context::{UiLayout, ViewerContext};

mod annotation_context_ui;
mod app_id_ui;
mod blob_ui;
mod component_path_ui;
mod component_type_ui;
mod component_ui;
mod component_ui_registry;
mod data_source_ui;
mod entity_db_ui;
mod entity_path_ui;
mod image_ui;
mod instance_path_ui;
mod store_id_ui;
mod tensor_ui;
mod transform_frames_ui;
mod video_ui;

mod extra_data_ui;
pub mod item_ui;

pub use self::component_ui::ComponentPathLatestAtResults;
pub use self::component_ui_registry::{add_to_registry, register_component_uis};
pub use self::image_ui::image_preview_ui;
pub use self::instance_path_ui::archetype_label_list_item_ui;
pub use self::tensor_ui::tensor_summary_ui_grid_contents;

use re_chunk_store::UnitChunkShared;
use re_types_core::reflection::Reflection;
use re_types_core::{ArchetypeName, Component};

pub type ArchetypeComponentMap =
    std::collections::BTreeMap<Option<ArchetypeName>, Vec<ComponentDescriptor>>;

/// Components grouped by archetype.
pub fn sorted_component_list_by_archetype_for_ui(
    reflection: &Reflection,
    iter: impl IntoIterator<Item = ComponentDescriptor>,
) -> ArchetypeComponentMap {
    let mut map = iter
        .into_iter()
        .fold(ArchetypeComponentMap::default(), |mut acc, descriptor| {
            acc.entry(descriptor.archetype)
                .or_default()
                .push(descriptor);
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
        ui.sanity_check();
        self.data_ui(ctx, ui, ui_layout, &ctx.current_query(), ctx.recording());
        ui.sanity_check();
    }
}

/// Similar to [`DataUi`], but for data that is related to an entity (e.g. a component).
///
/// This is given the context of the entity it is part of so it can do queries.
pub trait EntityDataUi {
    /// If you need to lookup something in the chunk store, use the given query to do so.
    #[expect(clippy::too_many_arguments)]
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
    db: &re_entity_db::EntityDb,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_entity_db::EntityPath,
) -> std::sync::Arc<re_viewer_context::Annotations> {
    re_tracing::profile_function!();
    let mut annotation_map = re_viewer_context::AnnotationMap::default();
    annotation_map.load(db, query);
    annotation_map.find(entity_path)
}

/// Finds and deserializes the given component type if its descriptor matches the given archetype name.
fn find_and_deserialize_archetype_mono_component<C: Component>(
    components: &[(ComponentDescriptor, UnitChunkShared)],
    archetype_name: Option<ArchetypeName>,
) -> Option<C> {
    components.iter().find_map(|(descr, chunk)| {
        (descr.component_type == Some(C::name()) && descr.archetype == archetype_name)
            .then(|| chunk.component_mono::<C>(descr.component)?.ok())
            .flatten()
    })
}
