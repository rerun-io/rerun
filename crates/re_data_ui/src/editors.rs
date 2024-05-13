// TODO(jleibs): Turn this into a trait

use egui::NumExt as _;
use re_data_store::LatestAtQuery;
use re_entity_db::{external::re_query::LatestAtComponentResults, EntityDb};
use re_log_types::{EntityPath, Instance};
use re_types::{
    components::{
        Color, MarkerShape, MarkerSize, Name, Radius, ScalarScattering, StrokeWidth, Text,
    },
    Component, Loggable,
};
use re_viewer_context::{UiLayout, ViewerContext};

// ----

#[allow(clippy::too_many_arguments)]
fn edit_color_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _ui_layout: UiLayout,
    query: &LatestAtQuery,
    db: &EntityDb,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &LatestAtComponentResults,
    instance: &Instance,
) {
    let current_color = component
        // TODO(#5607): what should happen if the promise is still pending?
        .instance::<Color>(db.resolver(), instance.get() as _)
        .unwrap_or_else(|| default_color(ctx, query, db, entity_path));

    let current_color = current_color.into();
    let mut edit_color = current_color;

    egui::color_picker::color_edit_button_srgba(
        ui,
        &mut edit_color,
        egui::color_picker::Alpha::Opaque,
    );

    if edit_color != current_color {
        ctx.save_blueprint_component(override_path, &Color::from(edit_color));
    }
}

#[inline]
fn default_color(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _db: &EntityDb,
    _entity_path: &EntityPath,
) -> Color {
    Color::from_rgb(255, 255, 255)
}

// ----

#[allow(clippy::too_many_arguments)]
fn edit_text_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _ui_layout: UiLayout,
    query: &LatestAtQuery,
    db: &EntityDb,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &LatestAtComponentResults,
    instance: &Instance,
) {
    let current_text = component
        // TODO(#5607): what should happen if the promise is still pending?
        .instance::<Text>(db.resolver(), instance.get() as _)
        .unwrap_or_else(|| default_text(ctx, query, db, entity_path));

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
    _db: &EntityDb,
    entity_path: &EntityPath,
) -> Text {
    Text::from(entity_path.to_string())
}

// ----
#[allow(clippy::too_many_arguments)]
fn edit_name_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _ui_layout: UiLayout,
    query: &LatestAtQuery,
    db: &EntityDb,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &LatestAtComponentResults,
    instance: &Instance,
) {
    let current_text = component
        // TODO(#5607): what should happen if the promise is still pending?
        .instance::<Name>(db.resolver(), instance.get() as _)
        .unwrap_or_else(|| default_name(ctx, query, db, entity_path));

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
    _db: &EntityDb,
    entity_path: &EntityPath,
) -> Name {
    Name::from(entity_path.to_string())
}

// ----

#[allow(clippy::too_many_arguments)]
fn edit_scatter_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _ui_layout: UiLayout,
    query: &LatestAtQuery,
    db: &EntityDb,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &LatestAtComponentResults,
    instance: &Instance,
) {
    let current_scatter = component
        // TODO(#5607): what should happen if the promise is still pending?
        .instance::<ScalarScattering>(db.resolver(), instance.get() as _)
        .unwrap_or_else(|| default_scatter(ctx, query, db, entity_path));

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
    _db: &EntityDb,
    _entity_path: &EntityPath,
) -> ScalarScattering {
    ScalarScattering::from(false)
}

// ----

#[allow(clippy::too_many_arguments)]
fn edit_radius_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _ui_layout: UiLayout,
    query: &LatestAtQuery,
    db: &EntityDb,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &LatestAtComponentResults,
    instance: &Instance,
) {
    let current_radius = component
        // TODO(#5607): what should happen if the promise is still pending?
        .instance::<Radius>(db.resolver(), instance.get() as _)
        .unwrap_or_else(|| default_radius(ctx, query, db, entity_path));

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
    _db: &EntityDb,
    _entity_path: &EntityPath,
) -> Radius {
    Radius::from(1.0)
}

// ----

#[allow(clippy::too_many_arguments)]
fn edit_marker_shape_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _ui_layout: UiLayout,
    query: &LatestAtQuery,
    db: &EntityDb,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &LatestAtComponentResults,
    instance: &Instance,
) {
    let current_marker = component
        // TODO(#5607): what should happen if the promise is still pending?
        .instance::<MarkerShape>(db.resolver(), instance.get() as _)
        .unwrap_or_else(|| default_marker_shape(ctx, query, db, entity_path));

    let mut edit_marker = current_marker;

    let marker_text = edit_marker.to_string();

    let item_width = 100.0;

    egui::ComboBox::from_id_source("marker_shape")
        .selected_text(marker_text) // TODO(emilk): Show marker shape in the selected text
        .width(
            ui.available_width()
                .at_most(item_width + ui.spacing().menu_margin.sum().x),
        )
        .height(320.0)
        .show_ui(ui, |ui| {
            // workaround to force `ui.max_rect()` to reflect the content size
            ui.set_width(item_width);

            let background_x_range = ui
                .spacing()
                .menu_margin
                .expand_rect(ui.max_rect())
                .x_range();

            let list_ui = |ui: &mut egui::Ui| {
                for marker in MarkerShape::ALL {
                    let response = ctx
                        .re_ui
                        .list_item2()
                        .selected(edit_marker == marker)
                        .show_flat(
                            ui,
                            re_ui::list_item2::LabelContent::new(marker.to_string())
                                .min_desired_width(item_width)
                                .with_icon_fn(|_re_ui, ui, rect, visuals| {
                                    paint_marker(ui, marker.into(), rect, visuals.text_color());
                                }),
                        );

                    if response.clicked() {
                        edit_marker = marker;
                    }
                }
            };

            re_ui::full_span::full_span_scope(ui, background_x_range, |ui| {
                re_ui::list_item2::list_item_scope(ui, "marker_shape", list_ui);
            });
        });

    if edit_marker != current_marker {
        ctx.save_blueprint_component(override_path, &edit_marker);
    }
}

#[inline]
fn default_marker_shape(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _db: &EntityDb,
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
    _ui_layout: UiLayout,
    query: &LatestAtQuery,
    db: &EntityDb,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &LatestAtComponentResults,
    instance: &Instance,
) {
    let current_stroke_width = component
        // TODO(#5607): what should happen if the promise is still pending?
        .instance::<StrokeWidth>(db.resolver(), instance.get() as _)
        .unwrap_or_else(|| default_stroke_width(ctx, query, db, entity_path));

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
    _db: &EntityDb,
    _entity_path: &EntityPath,
) -> StrokeWidth {
    StrokeWidth::from(1.0)
}

// ----

#[allow(clippy::too_many_arguments)]
fn edit_marker_size_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _ui_layout: UiLayout,
    query: &LatestAtQuery,
    db: &EntityDb,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &LatestAtComponentResults,
    instance: &Instance,
) {
    let current_marker_size = component
        // TODO(#5607): what should happen if the promise is still pending?
        .instance::<MarkerSize>(db.resolver(), instance.get() as _)
        .unwrap_or_else(|| default_marker_size(ctx, query, db, entity_path));

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
    _db: &EntityDb,
    _entity_path: &EntityPath,
) -> MarkerSize {
    MarkerSize::from(1.0)
}

// ----

fn register_editor<'a, C>(
    registry: &mut re_viewer_context::ComponentUiRegistry,
    default: fn(&ViewerContext<'_>, &LatestAtQuery, &EntityDb, &EntityPath) -> C,
    edit: fn(
        &ViewerContext<'_>,
        &mut egui::Ui,
        UiLayout,
        &LatestAtQuery,
        &EntityDb,
        &EntityPath,
        &EntityPath,
        &LatestAtComponentResults,
        &Instance,
    ),
) where
    C: Component + Loggable + 'static + Into<::std::borrow::Cow<'a, C>>,
{
    registry.add_editor(
        C::name(),
        Box::new(move |ctx, query, db, entity_path| {
            let c = default(ctx, query, db, entity_path);
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
