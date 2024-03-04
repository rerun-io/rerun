// TODO(jleibs): Turn this into a trait

use egui::NumExt as _;
use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::EntityPath;
use re_query::ComponentWithInstances;
use re_types::{
    components::{
        Color, MarkerShape, MarkerSize, Name, Radius, ScalarScattering, StrokeWidth, Text,
    },
    Component, Loggable,
};
use re_viewer_context::{UiVerbosity, ViewerContext};

// ----

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

        ctx.save_blueprint_component(override_path, &new_color);
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

// ----

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

        ctx.save_blueprint_component(override_path, &new_text);
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

// ----
#[allow(clippy::too_many_arguments)]
fn edit_name_ui(
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
        .lookup::<Name>(instance_key)
        .ok()
        .unwrap_or_else(|| default_name(ctx, query, store, entity_path));

    let current_text = current_text.to_string();
    let mut edit_text = current_text.clone();

    egui::TextEdit::singleline(&mut edit_text).show(ui);

    if edit_text != current_text {
        let new_text = Name::from(edit_text);

        ctx.save_blueprint_component(override_path, &new_text);
    }
}

#[inline]
fn default_name(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _store: &DataStore,
    entity_path: &EntityPath,
) -> Name {
    Name::from(entity_path.to_string())
}

// ----

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

        ctx.save_blueprint_component(override_path, &new_scatter);
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

// ----

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

        ctx.save_blueprint_component(override_path, &new_radius);
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

// ----

#[allow(clippy::too_many_arguments)]
fn edit_marker_shape_ui(
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
    let current_marker = component
        .lookup::<MarkerShape>(instance_key)
        .ok()
        .unwrap_or_else(|| default_marker_shape(ctx, query, store, entity_path));

    let mut edit_marker = current_marker;

    let marker_text = edit_marker.to_string();

    egui::ComboBox::from_id_source("marker_shape")
        .selected_text(marker_text) // TODO(emilk): Show marker shape in the selected text
        .width(100.0)
        .height(320.0)
        .show_ui(ui, |ui| {
            // Hack needed for ListItem to click its highlight bg rect correctly:
            ui.set_clip_rect(
                ui.clip_rect()
                    .with_max_x(ui.max_rect().max.x + ui.spacing().menu_margin.right),
            );

            for marker in MarkerShape::ALL {
                let list_item = re_ui::list_item::ListItem::new(ctx.re_ui, marker.to_string())
                    .with_icon_fn(|_re_ui, ui, rect, visuals| {
                        paint_marker(ui, marker.into(), rect, visuals.text_color());
                    })
                    .selected(edit_marker == marker);
                if list_item.show(ui).clicked() {
                    edit_marker = marker;
                }
            }
        });

    if edit_marker != current_marker {
        ctx.save_blueprint_component(override_path, &edit_marker);
    }
}

#[inline]
fn default_marker_shape(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _store: &DataStore,
    _entity_path: &EntityPath,
) -> MarkerShape {
    MarkerShape::default()
}

fn paint_marker(
    ui: &egui::Ui,
    marker: egui_plot::MarkerShape,
    rect: egui::Rect,
    color: egui::Color32,
) {
    use egui_plot::PlotItem as _;

    let points = egui_plot::Points::new([0.0, 0.0])
        .shape(marker)
        .color(color)
        .radius(rect.size().min_elem() / 2.0)
        .filled(true);

    let bounds = egui_plot::PlotBounds::new_symmetrical(0.5);
    let transform = egui_plot::PlotTransform::new(rect, bounds, true, true);

    let mut shapes = vec![];
    points.shapes(ui, &transform, &mut shapes);
    ui.painter().extend(shapes);
}

// ----

#[allow(clippy::too_many_arguments)]
fn edit_stroke_width_ui(
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
    let current_stroke_width = component
        .lookup::<StrokeWidth>(instance_key)
        .ok()
        .unwrap_or_else(|| default_stroke_width(ctx, query, store, entity_path));

    let current_stroke_width = current_stroke_width.0;
    let mut edit_stroke_width = current_stroke_width;

    let speed = (current_stroke_width * 0.01).at_least(0.001);

    ui.add(
        egui::DragValue::new(&mut edit_stroke_width)
            .clamp_range(0.0..=f64::INFINITY)
            .speed(speed),
    );

    if edit_stroke_width != current_stroke_width {
        let new_stroke_width = StrokeWidth::from(edit_stroke_width);

        ctx.save_blueprint_component(override_path, &new_stroke_width);
    }
}

#[inline]
fn default_stroke_width(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _store: &DataStore,
    _entity_path: &EntityPath,
) -> StrokeWidth {
    StrokeWidth::from(1.0)
}

// ----

#[allow(clippy::too_many_arguments)]
fn edit_marker_size_ui(
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
    let current_marker_size = component
        .lookup::<MarkerSize>(instance_key)
        .ok()
        .unwrap_or_else(|| default_marker_size(ctx, query, store, entity_path));

    let current_marker_size = current_marker_size.0;
    let mut edit_marker_size = current_marker_size;

    let speed = (current_marker_size * 0.01).at_least(0.001);

    ui.add(
        egui::DragValue::new(&mut edit_marker_size)
            .clamp_range(0.0..=f64::INFINITY)
            .speed(speed),
    );

    if edit_marker_size != current_marker_size {
        let new_marker_size = MarkerSize::from(edit_marker_size);

        ctx.save_blueprint_component(override_path, &new_marker_size);
    }
}

#[inline]
fn default_marker_size(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _store: &DataStore,
    _entity_path: &EntityPath,
) -> MarkerSize {
    MarkerSize::from(1.0)
}

// ----

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
    register_editor::<MarkerShape>(registry, default_marker_shape, edit_marker_shape_ui);
    register_editor::<MarkerSize>(registry, default_marker_size, edit_marker_size_ui);
    register_editor::<Name>(registry, default_name, edit_name_ui);
    register_editor::<Radius>(registry, default_radius, edit_radius_ui);
    register_editor::<ScalarScattering>(registry, default_scatter, edit_scatter_ui);
    register_editor::<StrokeWidth>(registry, default_stroke_width, edit_stroke_width_ui);
    register_editor::<Text>(registry, default_text, edit_text_ui);
}
