//! The `DataUi` trait and implementations provide methods for representing data using [`egui`].

use itertools::Itertools;
use re_log_types::{DataCell, EntityPath, PathOp, TimePoint};
use re_viewer_context::{UiVerbosity, ViewerContext};

mod annotation_context;
mod component;
mod component_path;
mod component_ui_registry;
mod data;
mod entity_path;
pub(crate) mod image;
mod instance_path;
mod log_msg;

pub use component_ui_registry::create_component_ui_registry;

use super::item_ui;

/// Types implementing [`DataUi`] can display themselves in an [`egui::Ui`].
pub trait DataUi {
    /// If you need to lookup something in the data store, use the given query to do so.
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    );
}

/// Similar to [`DataUi`], but for data that is related to an entity (e.g. a component).
///
/// This is given the context of the entity it is part of so it can do queries.
pub trait EntityDataUi {
    /// If you need to lookup something in the data store, use the given query to do so.
    fn entity_data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        entity_path: &EntityPath,
        query: &re_arrow_store::LatestAtQuery,
    );
}

impl<T> EntityDataUi for T
where
    T: DataUi,
{
    fn entity_data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _entity: &EntityPath,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        self.data_ui(ctx, ui, verbosity, query);
    }
}

// ----------------------------------------------------------------------------

impl DataUi for TimePoint {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
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
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        let mut sorted = self.to_vec();
        sorted.sort_by_key(|cb| cb.component_name());

        match verbosity {
            UiVerbosity::Small => {
                ui.label(sorted.iter().map(format_cell).join(", "));
            }

            UiVerbosity::All | UiVerbosity::Reduced => {
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

impl DataUi for PathOp {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            PathOp::ClearComponents(entity_path) => {
                ui.label(format!("ClearComponents: {entity_path}"))
            }
            PathOp::ClearRecursive(entity_path) => {
                ui.label(format!("ClearRecursive: {entity_path}"))
            }
        };
    }
}
