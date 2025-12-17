use re_sdk_types::blueprint::components::ZoomLevel;
use re_viewer_context::MaybeMutRef;

// TODO(#7876): move this to `re_view_map` when the crate is no longer behind a Cargo feature.
// TODO(ab): currently set at 19 because that's what walkers has as hard-coded limit. In the future,
// walkers will need to be more flexible (e.g. depend on the actually max zoom level for the map
// provider). At that point, we will have to set some kind of "max ever" value here.
const MAX_ZOOM_LEVEL: f64 = 19.0;

/// Editor for a [`re_sdk_types::blueprint::components::ZoomLevel`].
pub fn edit_zoom_level(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, ZoomLevel>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, f64> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(&mut value.0),
    };

    super::datatype_uis::edit_f64_float_raw_with_speed_impl(
        ui,
        &mut value,
        0.0..=MAX_ZOOM_LEVEL,
        0.1,
    )
}
