use std::ops::RangeInclusive;

use egui::NumExt as _;
use egui::emath::Numeric;
use re_types_core::datatypes;
use re_ui::syntax_highlighting::{SyntaxHighlightedBuilder, SyntaxHighlighting};
use re_viewer_context::{MaybeMutRef, UiLayout};

/// Generic editor for a [`re_sdk_types::datatypes::UInt32`] values within a given range.
pub fn edit_u32_range(
    _ctx: &re_viewer_context::AppContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = datatypes::UInt32>>,
    range: RangeInclusive<u32>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, u32> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(&mut value.deref_mut().0),
    };
    edit_int_raw(ui, &mut value, range, "")
}

/// Generic editor for a [`re_sdk_types::datatypes::UInt64`] values within a given range.
pub fn edit_u64_range(
    _ctx: &re_viewer_context::AppContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = datatypes::UInt64>>,
    range: RangeInclusive<u64>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, u64> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(&mut value.deref_mut().0),
    };
    edit_int_raw(ui, &mut value, range, "")
}

/// Non monomorphized implementation for integer editing.
pub fn edit_int_raw<T: Numeric + SyntaxHighlighting>(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, T>,
    range: RangeInclusive<T>,
    suffix: &str,
) -> egui::Response {
    // TODO(emilk): we could do something even smarter here, but for now this is good enough.
    let max_pts_per_step = 20.0; // A matter of taste
    let min_speed = 1.0 / max_pts_per_step;

    let use_exponential_speed = 50.0 < range.end().to_f64() - range.start().to_f64();

    let speed = if use_exponential_speed {
        ((**value).to_f64() * 0.01).at_least(min_speed)
    } else {
        min_speed
    };
    edit_int_raw_with_speed_impl(ui, value, range, speed, suffix)
}

/// Non monomorphized implementation for integer editing with a given speed.
pub fn edit_int_raw_with_speed_impl<T: Numeric + SyntaxHighlighting>(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, T>,
    range: RangeInclusive<T>,
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
        UiLayout::List.data_label(
            ui,
            SyntaxHighlightedBuilder::new()
                .with(&**value)
                .with_primitive(suffix),
        )
    }
}
