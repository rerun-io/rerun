//! Rerun Data Ui
//!
//! This crate provides ui elements for Rerun component data for the Rerun Viewer.

use itertools::Itertools;

use re_log_types::{DataCell, EntityPath, TimePoint};
use re_types::ComponentName;
use re_viewer_context::{UiVerbosity, ViewerContext};

mod annotation_context;
mod blueprint_data;
mod component;
mod component_path;
mod component_ui_registry;
mod data;
mod entity_path;
mod image;
mod image_meaning;
mod instance_path;
pub mod item_ui;
mod log_msg;
mod pinhole;
mod rotation3d;
mod transform3d;

pub use crate::image::{
    show_zoomed_image_region, show_zoomed_image_region_area_outline,
    tensor_summary_ui_grid_contents,
};
pub use component_ui_registry::{add_to_registry, create_component_ui_registry};
pub use image_meaning::image_meaning_for_entity;

/// Filter out components that should not be shown in the UI,
/// and order the other components in a cosnsiten way.
pub fn ui_visible_components<'a>(
    iter: impl IntoIterator<Item = &'a ComponentName> + 'a,
) -> impl Iterator<Item = &'a ComponentName> {
    let mut components: Vec<&ComponentName> = iter
        .into_iter()
        .filter(|c| is_component_visible_in_ui(c))
        .collect();

    // Put indicator components first:
    components.sort_by_key(|c| (!c.is_indicator_component(), c.full_name()));

    components.into_iter()
}

/// Show this component in the UI.
fn is_component_visible_in_ui(component_name: &ComponentName) -> bool {
    const HIDDEN_COMPONENTS: &[&str] = &["rerun.components.InstanceKey"];
    !HIDDEN_COMPONENTS.contains(&component_name.as_ref())
}

pub fn temporary_style_ui_for_component<R>(
    ui: &mut egui::Ui,
    component_name: &ComponentName,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let old_style: egui::Style = (**ui.style()).clone();

    if component_name.is_indicator_component() {
        // Make indicator components stand out by making them slightly fainter:

        let inactive = &mut ui.style_mut().visuals.widgets.inactive;
        // TODO(emilk): get a color from the design-tokens
        inactive.fg_stroke.color = inactive.fg_stroke.color.linear_multiply(0.45);
    }

    let ret = add_contents(ui);

    ui.set_style(old_style);

    ret
}

/// Types implementing [`DataUi`] can display themselves in an [`egui::Ui`].
pub trait DataUi {
    /// If you need to lookup something in the data store, use the given query to do so.
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
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
        verbosity: UiVerbosity,
        entity_path: &EntityPath,
        query: &re_data_store::LatestAtQuery,
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
        verbosity: UiVerbosity,
        entity: &EntityPath,
        query: &re_data_store::LatestAtQuery,
    ) {
        // This ensures that UI state is maintained per entity. For example, the collapsed state for
        // `AnnotationContext` component is not saved by all instances of the component.
        ui.push_id(entity.hash(), |ui| {
            self.data_ui(ctx, ui, verbosity, query);
        });
    }
}

// ----------------------------------------------------------------------------

impl DataUi for TimePoint {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
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
        verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
    ) {
        let mut sorted = self.to_vec();
        sorted.sort_by_key(|cb| cb.component_name());

        match verbosity {
            UiVerbosity::Small => {
                ui.label(sorted.iter().map(format_cell).join(", "));
            }

            UiVerbosity::Full | UiVerbosity::LimitHeight | UiVerbosity::Reduced => {
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

// ---------------------------------------------------------------------------

/// Build an egui table and configure it for the given verbosity.
///
/// Note that the caller is responsible for strictly limiting the number of displayed rows for
/// [`UiVerbosity::Small`] and [`UiVerbosity::Reduced`], as the table will not scroll.
pub fn table_for_verbosity(
    verbosity: UiVerbosity,
    ui: &mut egui::Ui,
) -> egui_extras::TableBuilder<'_> {
    let table = egui_extras::TableBuilder::new(ui);
    match verbosity {
        UiVerbosity::Small | UiVerbosity::Reduced => {
            // Be as small as possible in the hover tooltips. No scrolling related configuration, as
            // the content itself must be limited (scrolling is not possible in tooltips).
            table.auto_shrink([true, true])
        }
        UiVerbosity::LimitHeight => {
            // Don't take too much vertical space to leave room for other selected items.
            table
                .auto_shrink([false, true])
                .vscroll(true)
                .max_scroll_height(100.0)
        }
        UiVerbosity::Full => {
            // We're alone in the selection panel. Let the outer ScrollArea do the work.
            table.auto_shrink([false, true]).vscroll(false)
        }
    }
}
