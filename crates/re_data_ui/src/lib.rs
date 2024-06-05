//! Rerun Data Ui
//!
//! This crate provides ui elements for Rerun component data for the Rerun Viewer.

use itertools::Itertools;

use re_log_types::{DataCell, EntityPath, TimePoint};
use re_types::ComponentName;
use re_viewer_context::{UiLayout, ViewerContext};

mod annotation_context;
mod app_id;
mod blueprint_data;
mod blueprint_types;
mod component;
mod component_path;
mod component_ui_registry;
mod data;
mod data_source;
mod entity_db;
mod entity_path;
mod image;
mod image_meaning;
mod instance_path;
mod log_msg;
mod material;
mod pinhole;
mod rotation3d;
mod store_id;
mod transform3d;

pub mod item_ui;

pub use crate::image::{
    show_zoomed_image_region, show_zoomed_image_region_area_outline,
    tensor_summary_ui_grid_contents,
};
pub use component::EntityLatestAtResults;
pub use component_ui_registry::{add_to_registry, create_component_ui_registry};
pub use image_meaning::image_meaning_for_entity;

/// Sort components for display in the UI.
pub fn component_list_for_ui<'a>(
    iter: impl IntoIterator<Item = &'a ComponentName> + 'a,
) -> Vec<ComponentName> {
    let mut components: Vec<ComponentName> = iter.into_iter().copied().collect();

    // Put indicator components first:
    components.sort_by_key(|c| (!c.is_indicator_component(), c.full_name()));

    components
}

/// Types implementing [`DataUi`] can display themselves in an [`egui::Ui`].
pub trait DataUi {
    /// If you need to lookup something in the data store, use the given query to do so.
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    );
}

/// Similar to [`DataUi`], but for data that is related to an entity (e.g. a component).
///
/// This is given the context of the entity it is part of so it can do queries.
pub trait EntityDataUi {
    /// If you need to lookup something in the data store, use the given query to do so.
    fn entity_data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        entity_path: &EntityPath,
        query: &re_data_store::LatestAtQuery,
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
        query: &re_data_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        // This ensures that UI state is maintained per entity. For example, the collapsed state for
        // `AnnotationContext` component is not saved by all instances of the component.
        ui.push_id(entity_path.hash(), |ui| {
            self.data_ui(ctx, ui, ui_layout, query, db);
        });
    }
}

// ----------------------------------------------------------------------------

impl DataUi for TimePoint {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui.vertical(|ui| {
            egui::Grid::new("time_point").num_columns(2).show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (timeline, value) in self.iter() {
                    item_ui::timeline_button_to(ctx, ui, format!("{}:", timeline.name()), timeline);
                    item_ui::time_button(ctx, ui, timeline, *value);
                    ui.end_row();
                }
            });
        });
    }
}

impl DataUi for [DataCell] {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        let mut sorted = self.to_vec();
        sorted.sort_by_key(|cb| cb.component_name());

        match ui_layout {
            UiLayout::List => {
                ui.label(sorted.iter().map(format_cell).join(", "));
            }

            UiLayout::SelectionPanelFull
            | UiLayout::SelectionPanelLimitHeight
            | UiLayout::Tooltip => {
                ui.vertical(|ui| {
                    for component_bundle in &sorted {
                        ui.label(format_cell(component_bundle));
                    }
                });
            }
        }
    }
}

fn format_cell(cell: &DataCell) -> String {
    // TODO(emilk): if there's only once instance, and the byte size is small, then deserialize and show the value.
    format!(
        "{}x {}",
        cell.num_instances(),
        cell.component_name().short_name()
    )
}

// ---------------------------------------------------------------------------

pub fn annotations(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    entity_path: &re_entity_db::EntityPath,
) -> std::sync::Arc<re_viewer_context::Annotations> {
    re_tracing::profile_function!();
    let mut annotation_map = re_viewer_context::AnnotationMap::default();
    annotation_map.load(ctx, query, std::iter::once(entity_path));
    annotation_map.find(entity_path)
}
