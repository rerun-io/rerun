use arrow::array::{Array as _, BooleanArray};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_types_core::{ComponentIdentifier, RowId};
use re_ui::UiExt as _;
use re_viewer_context::{AppContext, MaybeMutRef};

/// Show a scalar boolean as a checkbox and return its toggled value.
///
/// A null value is shown as an indeterminate checkbox, and checking it sets it to `true`.
/// The checkbox is named after the component, which is the column name in tables.
pub fn checkbox(
    _ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    component: ComponentIdentifier,
    _row_id: Option<RowId>,
    value: &mut MaybeMutRef<'_, arrow::array::ArrayRef>,
) -> Result<egui::Response, Box<dyn std::error::Error>> {
    let bools = value
        .as_ref()
        .downcast_array_ref::<BooleanArray>()
        .ok_or("A checkbox requires boolean data")?;
    if bools.len() != 1 {
        return Err("A checkbox requires one scalar value".into());
    }

    let is_null = bools.is_null(0);
    let mut checked = !is_null && bools.value(0);
    let enabled = value.as_mut().is_some();
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ui.checkbox_indeterminate(&mut checked, "", is_null)
        })
        .inner;
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, checked, component.as_str())
    });

    if response.changed()
        && let Some(value) = value.as_mut()
    {
        *value = std::sync::Arc::new(BooleanArray::from(vec![Some(checked)]));
    }
    Ok(response)
}
