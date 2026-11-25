use egui::color_picker::{Alpha, color_picker_color32};
use egui::{Color32, Popup, PopupCloseBehavior, Response, Role, Ui, WidgetInfo};
use re_sdk_types::components::AnnotationContext;
use re_sdk_types::encodings::{AnnotationInfo, Rgba32};
use re_viewer_context::{MaybeMutRef, auto_color_egui};

use crate::color_swatch::ColorSwatch;

/// Edits the color and visibility of each class.
///
/// A hidden class is fully transparent.
pub fn edit_or_view_annotation_context(
    _ctx: &re_viewer_context::AppContext<'_>,
    ui: &mut Ui,
    context: &mut MaybeMutRef<'_, AnnotationContext>,
) -> Response {
    let summary = format!("{} classes", re_format::format_uint(context.0.len()));
    let Some(context) = context.as_mut() else {
        return ui.label(summary);
    };

    let mut response = ui.button(summary);
    let mut changed = false;
    Popup::menu(&response)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            for class in &mut context.0 {
                changed |= class_ui(ui, &mut class.class_description.info).changed();
            }
        });
    if changed {
        response.mark_changed();
    }
    response
}

/// Clicking the swatch toggles a picker of the class's RGB below its row: the editor is already a
/// popup, which can't open another one.
///
/// A hidden class keeps its RGB, so showing it again restores its color, fully opaque.
fn class_ui(ui: &mut Ui, info: &mut AnnotationInfo) -> Response {
    let [r, g, b, alpha] = info
        .color
        .unwrap_or_else(|| auto_color_egui(info.id).into())
        .to_array();
    let mut visible = alpha > 0;
    #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
    let mut rgb = Color32::from_rgb(r, g, b);
    let label = info
        .label
        .as_ref()
        .map_or_else(|| info.id.to_string(), |label| label.as_str().to_owned());
    let picker_id = ui.make_persistent_id(("class_color_picker", info.id));
    let mut picker_open = ui.data(|data| data.get_temp(picker_id)).unwrap_or(false);

    let mut response = ui
        .horizontal(|ui| {
            let swatch_color = Rgba32::from_rgb(r, g, b);
            let swatch = ui.add(ColorSwatch::new(&mut MaybeMutRef::Ref(&swatch_color)));
            swatch.widget_info(|| WidgetInfo::labeled(Role::ColorWell, true, label.as_str()));
            if swatch.clicked() {
                picker_open = !picker_open;
            }
            ui.checkbox(&mut visible, label.as_str())
        })
        .inner;
    if picker_open && color_picker_color32(ui, &mut rgb, Alpha::Opaque) {
        response.mark_changed();
    }
    ui.data_mut(|data| data.insert_temp(picker_id, picker_open));

    if response.changed() {
        let [r, g, b, _] = rgb.to_array();
        let alpha = match (visible, alpha) {
            (false, _) => 0,
            (true, 0) => u8::MAX,
            (true, alpha) => alpha,
        };
        info.color = Some(Rgba32::from_unmultiplied_rgba(r, g, b, alpha));
    }
    response
}
