pub(crate) mod context_panel;
pub(crate) mod data_ui;
pub(crate) mod image_ui;
pub(crate) mod log_table_view;
pub(crate) mod space_view;
pub(crate) mod time_panel;
pub(crate) mod view2d;

#[cfg(feature = "wgpu")]
pub(crate) mod view_tensor;

#[cfg(feature = "glow")]
pub(crate) mod view3d;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Preview {
    Small,
    Medium,
    Specific(f32),
}
