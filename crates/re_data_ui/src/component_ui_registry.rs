use re_data_store::LatestAtQuery;
use re_entity_db::{external::re_query::LatestAtComponentResults, EntityDb};
use re_log_types::{external::arrow2, EntityPath, Instance};
use re_types::external::arrow2::array::Utf8Array;
use re_viewer_context::{ComponentUiRegistry, UiVerbosity, ViewerContext};

use crate::editors::register_editors;

use super::EntityDataUi;

pub fn create_component_ui_registry() -> ComponentUiRegistry {
    re_tracing::profile_function!();

    let mut registry = ComponentUiRegistry::new(Box::new(&fallback_component_ui));

    add_to_registry::<re_types::components::AnnotationContext>(&mut registry);
    add_to_registry::<re_types::components::ClassId>(&mut registry);
    add_to_registry::<re_types::components::Color>(&mut registry);
    add_to_registry::<re_types::components::PinholeProjection>(&mut registry);
    add_to_registry::<re_types::components::KeypointId>(&mut registry);
    add_to_registry::<re_types::components::LineStrip2D>(&mut registry);
    add_to_registry::<re_types::components::LineStrip3D>(&mut registry);
    add_to_registry::<re_types::components::Resolution>(&mut registry);
    add_to_registry::<re_types::components::Rotation3D>(&mut registry);
    add_to_registry::<re_types::components::Material>(&mut registry);
    add_to_registry::<re_types::components::TensorData>(&mut registry);
    add_to_registry::<re_types::components::Transform3D>(&mut registry);
    add_to_registry::<re_types::components::OutOfTreeTransform3D>(&mut registry);
    add_to_registry::<re_types::components::ViewCoordinates>(&mut registry);

    add_to_registry::<re_types_blueprint::blueprint::components::IncludedSpaceView>(&mut registry);
    add_to_registry::<re_types_blueprint::blueprint::components::SpaceViewMaximized>(&mut registry);

    register_editors(&mut registry);

    registry
}

/// Registers how to show a given component in the ui.
pub fn add_to_registry<C: EntityDataUi + re_types::Component>(registry: &mut ComponentUiRegistry) {
    registry.add(
        C::name(),
        Box::new(
            |ctx, ui, verbosity, query, db, entity_path, component, instance| {
                // TODO(#5607): what should happen if the promise is still pending?
                if let Some(component) = component.instance::<C>(db.resolver(), instance.get() as _)
                {
                    component.entity_data_ui(ctx, ui, verbosity, entity_path, query, db);
                } else {
                    ui.weak("(not found)");
                }
            },
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn fallback_component_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    verbosity: UiVerbosity,
    _query: &LatestAtQuery,
    db: &EntityDb,
    _entity_path: &EntityPath,
    component: &LatestAtComponentResults,
    instance: &Instance,
) {
    // TODO(#5607): what should happen if the promise is still pending?
    let value = component
        .component_name(db.resolver())
        .and_then(|component_name| {
            component.instance_raw(db.resolver(), component_name, instance.get() as _)
        });

    // No special ui implementation - use a generic one:
    if let Some(value) = value {
        arrow_ui(ui, verbosity, &*value);
    } else {
        ui.weak("(null)");
    }
}

fn arrow_ui(ui: &mut egui::Ui, verbosity: UiVerbosity, array: &dyn arrow2::array::Array) {
    use re_types::SizeBytes as _;

    // Special-treat text.
    // Note: we match on the raw data here, so this works for any component containing text.
    if let Some(utf8) = array.as_any().downcast_ref::<Utf8Array<i32>>() {
        if utf8.len() == 1 {
            let string = utf8.value(0);
            text_ui(ui, verbosity, string);
            return;
        }
    }
    if let Some(utf8) = array.as_any().downcast_ref::<Utf8Array<i64>>() {
        if utf8.len() == 1 {
            let string = utf8.value(0);
            text_ui(ui, verbosity, string);
            return;
        }
    }

    let num_bytes = array.total_size_bytes();
    if num_bytes < 3000 {
        // Print small items:
        let mut string = String::new();
        let display = arrow2::array::get_display(array, "null");
        if display(&mut string, 0).is_ok() {
            text_ui(ui, verbosity, &string);
            return;
        }
    }

    // Fallback:
    let bytes = re_format::format_bytes(num_bytes as _);

    // TODO(emilk): pretty-print data type
    let data_type_formatted = format!("{:?}", array.data_type());

    if data_type_formatted.len() < 20 {
        // e.g. "4.2 KiB of Float32"
        text_ui(ui, verbosity, &format!("{bytes} of {data_type_formatted}"));
    } else {
        // Huge datatype, probably a union horror show
        ui.label(format!("{bytes} of data"));
    }
}

fn text_ui(ui: &mut egui::Ui, verbosity: UiVerbosity, string: &str) {
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
    let color = ui.visuals().text_color();
    let wrap_width = ui.available_width();
    let mut layout_job =
        egui::text::LayoutJob::simple(string.to_owned(), font_id, color, wrap_width);

    let mut needs_scroll_area = false;

    match verbosity {
        UiVerbosity::Small => {
            // Elide
            layout_job.wrap.max_rows = 1;
            layout_job.wrap.break_anywhere = true;
        }
        UiVerbosity::Reduced => {
            layout_job.wrap.max_rows = 3;
        }
        UiVerbosity::LimitHeight => {
            let num_newlines = string.chars().filter(|&c| c == '\n').count();
            needs_scroll_area = 10 < num_newlines || 300 < string.len();
        }
        UiVerbosity::Full => {
            needs_scroll_area = false;
        }
    }

    let galley = ui.fonts(|f| f.layout_job(layout_job)); // We control the text layout; not the label

    if needs_scroll_area {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(galley);
        });
    } else {
        ui.label(galley);
    }
}
