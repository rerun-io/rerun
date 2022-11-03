pub(crate) mod class_description_ui;
pub(crate) mod data_ui;
pub(crate) mod event_log_view;
pub(crate) mod image_ui;
pub(crate) mod kb_shortcuts;
pub(crate) mod legend;
pub(crate) mod scene;
pub(crate) mod selection_panel;
pub(crate) mod space_view;
pub(crate) mod text_entry_view;
pub(crate) mod time_panel;
pub(crate) mod view2d;
pub(crate) mod view3d;
pub(crate) mod view_tensor;
pub(crate) mod viewport;

pub(crate) use space_view::SpaceView;
pub(crate) use viewport::{Blueprint, SpaceViewId};

mod tensor_dimension_mapper;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Preview {
    Small,
    Medium,
    Specific(f32),
}
