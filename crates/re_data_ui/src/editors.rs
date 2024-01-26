// TODO(jleibs): Turn this into a trait

use egui::NumExt as _;
use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::EntityPath;
use re_query::ComponentWithInstances;
use re_types::{
    components::{Color, Radius, ScalarScattering, Text},
    Component, Loggable,
};
use re_viewer_context::{UiVerbosity, ViewerContext};

#[allow(clippy::too_many_arguments)]
fn edit_color_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _verbosity: UiVerbosity,
    query: &LatestAtQuery,
    store: &DataStore,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &ComponentWithInstances,
    instance_key: &re_types::components::InstanceKey,
) {
    let current_color = component
        .lookup::<Color>(instance_key)
        .ok()
        .unwrap_or_else(|| default_color(ctx, query, store, entity_path));

    let [r, g, b, a] = current_color.to_array();
    let current_color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
    let mut edit_color = current_color;

    egui::color_picker::color_edit_button_srgba(
        ui,
        &mut edit_color,
        egui::color_picker::Alpha::Opaque,
    );

    if edit_color != current_color {
        let [r, g, b, a] = edit_color.to_array();
        let new_color = Color::from_unmultiplied_rgba(r, g, b, a);

        ctx.save_blueprint_component(override_path, new_color);
    }
}

#[inline]
fn default_color(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _store: &DataStore,
    _entity_path: &EntityPath,
) -> Color {
    Color::from_rgb(255, 255, 255)
}

#[allow(clippy::too_many_arguments)]
fn edit_text_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _verbosity: UiVerbosity,
    query: &LatestAtQuery,
    store: &DataStore,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &ComponentWithInstances,
    instance_key: &re_types::components::InstanceKey,
) {
    let current_text = component
        .lookup::<Text>(instance_key)
        .ok()
        .unwrap_or_else(|| default_text(ctx, query, store, entity_path));

    let current_text = current_text.to_string();
    let mut edit_text = current_text.clone();

    egui::TextEdit::singleline(&mut edit_text).show(ui);

    if edit_text != current_text {
        let new_text = Text::from(edit_text);

        ctx.save_blueprint_component(override_path, new_text);
    }
}

#[inline]
fn default_text(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _store: &DataStore,
    entity_path: &EntityPath,
) -> Text {
    Text::from(entity_path.to_string())
}

#[allow(clippy::too_many_arguments)]
fn edit_scatter_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _verbosity: UiVerbosity,
    query: &LatestAtQuery,
    store: &DataStore,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &ComponentWithInstances,
    instance_key: &re_types::components::InstanceKey,
) {
    let current_scatter = component
        .lookup::<ScalarScattering>(instance_key)
        .ok()
        .unwrap_or_else(|| default_scatter(ctx, query, store, entity_path));

    let current_scatter = current_scatter.0;
    let mut edit_scatter = current_scatter;

    let scattered_text = if current_scatter { "Scattered" } else { "Line" };

    egui::ComboBox::from_id_source("scatter")
        .selected_text(scattered_text)
        .show_ui(ui, |ui| {
            ui.style_mut().wrap = Some(false);
            ui.selectable_value(&mut edit_scatter, false, "Line");
            ui.selectable_value(&mut edit_scatter, true, "Scattered");
        });

    if edit_scatter != current_scatter {
        let new_scatter = ScalarScattering::from(edit_scatter);

        ctx.save_blueprint_component(override_path, new_scatter);
    }
}

#[inline]
fn default_scatter(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _store: &DataStore,
    _entity_path: &EntityPath,
) -> ScalarScattering {
    ScalarScattering::from(false)
}

#[allow(clippy::too_many_arguments)]
fn edit_radius_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _verbosity: UiVerbosity,
    query: &LatestAtQuery,
    store: &DataStore,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &ComponentWithInstances,
    instance_key: &re_types::components::InstanceKey,
) {
    let current_radius = component
        .lookup::<Radius>(instance_key)
        .ok()
        .unwrap_or_else(|| default_radius(ctx, query, store, entity_path));

    let current_radius = current_radius.0;
    let mut edit_radius = current_radius;

    let speed = (current_radius * 0.01).at_least(0.001);

    ui.add(
        egui::DragValue::new(&mut edit_radius)
            .clamp_range(0.0..=f64::INFINITY)
            .speed(speed),
    );

    if edit_radius != current_radius {
        let new_radius = Radius::from(edit_radius);

        ctx.save_blueprint_component(override_path, new_radius);
    }
}

#[inline]
fn default_radius(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _store: &DataStore,
    _entity_path: &EntityPath,
) -> Radius {
    Radius::from(1.0)
}

fn register_editor<'a, C: Component + Loggable + 'static>(
    registry: &mut re_viewer_context::ComponentUiRegistry,
    default: fn(&ViewerContext<'_>, &LatestAtQuery, &DataStore, &EntityPath) -> C,
    edit: fn(
        &ViewerContext<'_>,
        &mut egui::Ui,
        UiVerbosity,
        &LatestAtQuery,
        &DataStore,
        &EntityPath,
        &EntityPath,
        &ComponentWithInstances,
        &re_types::components::InstanceKey,
    ),
) where
    C: Into<::std::borrow::Cow<'a, C>>,
{
    registry.add_editor(
        C::name(),
        Box::new(move |ctx, query, store, entity_path| {
            let c = default(ctx, query, store, entity_path);
            [c].into()
        }),
        Box::new(edit),
    );
}

pub fn register_editors(registry: &mut re_viewer_context::ComponentUiRegistry) {
    register_editor::<Color>(registry, default_color, edit_color_ui);
    register_editor::<Text>(registry, default_text, edit_text_ui);
    register_editor::<ScalarScattering>(registry, default_scatter, edit_scatter_ui);
    register_editor::<Radius>(registry, default_radius, edit_radius_ui);
}
