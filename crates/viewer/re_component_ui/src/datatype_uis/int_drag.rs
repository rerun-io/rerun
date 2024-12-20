use std::ops::RangeInclusive;

use egui::NumExt as _;
use re_types_core::datatypes;
use re_viewer_context::{MaybeMutRef, UiLayout};

/// Generic editor for a [`re_types::datatypes::UInt64`] values within a given range.
pub fn edit_u64_range(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = datatypes::UInt64>>,
    range: RangeInclusive<u64>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, u64> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(&mut value.deref_mut().0),
    };
    edit_u64_raw(ui, &mut value, range, "")
}

/// Non monomorphized implementation for u64 editing.
pub fn edit_u64_raw(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, u64>,
    range: RangeInclusive<u64>,
    suffix: &str,
) -> egui::Response {
    let speed = (**value as f64 * 0.01).at_least(0.001);
    edit_u64_raw_with_speed_impl(ui, value, range, speed, suffix)
}

/// Non monomorphized implementation for u64 editing with a given speed.
pub fn edit_u64_raw_with_speed_impl(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, u64>,
    range: RangeInclusive<u64>,
    speed: f64,
    suffix: &str,
) -> egui::Response {
    if let Some(value) = value.as_mut() {
        ui.add(
            egui::DragValue::new(value)
                .clamp_existing_to_range(false)
                .range(range)
                .speed(speed)
                .suffix(suffix),
        )
    } else {
        UiLayout::List.data_label(ui, format!("{}{}", re_format::format_uint(**value), suffix))
    }
}
