use std::collections::BTreeMap;

use re_log_types::{
    external::arrow2,
    external::arrow2_convert::{
        deserialize::{ArrowArray, ArrowDeserialize},
        field::ArrowField,
    },
    field_types::Instance,
    msg_bundle::Component,
    ComponentName,
};
use re_query::ComponentWithInstances;

use crate::{misc::ViewerContext, ui::Preview};

use super::DataUi;

type ComponentUiCallback =
    Box<dyn Fn(&mut ViewerContext<'_>, &mut egui::Ui, Preview, &ComponentWithInstances, &Instance)>;

/// How to display components in a Ui
pub(crate) struct ComponentUiRegistry {
    components: BTreeMap<ComponentName, ComponentUiCallback>,
}

impl Default for ComponentUiRegistry {
    fn default() -> Self {
        let mut registry = Self {
            components: Default::default(),
        };

        registry.add::<re_log_types::field_types::AnnotationContext>();
        // registry.add::<re_log_types::field_types::Arrow3D>();
        // registry.add::<re_log_types::field_types::Box3D>();
        // registry.add::<re_log_types::field_types::ClassId>();
        registry.add::<re_log_types::field_types::ColorRGBA>();
        // registry.add::<re_log_types::field_types::Instance>();
        // registry.add::<re_log_types::field_types::KeypointId>();
        // registry.add::<re_log_types::field_types::Label>();
        // registry.add::<re_log_types::field_types::LineStrip2D>();
        // registry.add::<re_log_types::field_types::LineStrip3D>();
        // registry.add::<re_log_types::field_types::Mesh3D>();
        registry.add::<re_log_types::field_types::MsgId>();
        // registry.add::<re_log_types::field_types::Point2D>();
        // registry.add::<re_log_types::field_types::Point3D>();
        // registry.add::<re_log_types::field_types::Quaternion>();
        // registry.add::<re_log_types::field_types::Radius>();
        // registry.add::<re_log_types::field_types::Rect2D>();
        // registry.add::<re_log_types::field_types::Scalar>();
        // registry.add::<re_log_types::field_types::ScalarPlotProps>();
        // registry.add::<re_log_types::field_types::Size3D>();
        // registry.add::<re_log_types::field_types::Tensor>();
        registry.add::<re_log_types::field_types::TextEntry>();
        registry.add::<re_log_types::field_types::Transform>();
        // registry.add::<re_log_types::field_types::Vec2D>();
        // registry.add::<re_log_types::field_types::Vec3D>();

        registry
    }
}

impl ComponentUiRegistry {
    fn add<C>(&mut self)
    where
        C: Component + DataUi + ArrowDeserialize + ArrowField<Type = C> + 'static,
        C::ArrayType: ArrowArray,
        for<'a> &'a C::ArrayType: IntoIterator,
    {
        self.components.insert(
            C::name(),
            Box::new(|ctx, ui, preview, component, instance| {
                match component.lookup::<C>(instance) {
                    Ok(component) => component.data_ui(ctx, ui, preview),
                    Err(re_query::QueryError::ComponentNotFound) => {
                        ui.weak("(not found)");
                    }
                    Err(err) => {
                        re_log::warn_once!("Expected component {}, {}", C::name(), err);
                    }
                }
            }),
        );
    }

    pub fn ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: crate::ui::Preview,
        component: &ComponentWithInstances,
        instance: &Instance,
    ) {
        if let Some(ui_callback) = self.components.get(&component.name()) {
            (*ui_callback)(ctx, ui, preview, component, instance);
        } else {
            // No special ui implementation - use a generic one:

            if let Some(value) = component.lookup_arrow(instance) {
                let bytes = arrow2::compute::aggregate::estimated_bytes_size(value.as_ref());
                if bytes < 256 {
                    // For small items, print them
                    let mut repr = String::new();
                    let display = arrow2::array::get_display(value.as_ref(), "null");
                    display(&mut repr, 0).unwrap();
                    ui.label(repr);
                } else {
                    ui.label(format!("{} bytes", bytes));
                }
            } else {
                ui.weak("(null)");
            }
        }
    }
}

// ----------------------------------------------------------------------------

impl DataUi for re_log_types::field_types::TextEntry {
    fn data_ui(&self, _ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, preview: Preview) {
        use crate::ui::view_text::level_to_rich_text;

        let Self { body, level } = self;

        match preview {
            Preview::Small | Preview::MaxHeight(_) => {
                ui.horizontal(|ui| {
                    if let Some(level) = level {
                        ui.label(level_to_rich_text(ui, level));
                    }
                    ui.label(format!("{body:?}")); // Debug format to get quotes and escapes
                });
            }
            Preview::Large => {
                egui::Grid::new("text_entry").num_columns(2).show(ui, |ui| {
                    ui.label("level:");
                    if let Some(level) = level {
                        ui.label(level_to_rich_text(ui, level));
                    }
                    ui.end_row();

                    ui.label("body:");
                    ui.label(format!("{body:?}")); // Debug format to get quotes and escapes
                    ui.end_row();
                });
            }
        }
    }
}
