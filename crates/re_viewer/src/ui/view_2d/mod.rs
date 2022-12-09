mod ui;
pub(crate) use self::ui::{view_2d, View2DState, HELP_TEXT};

mod class_description_ui;
pub(crate) use self::class_description_ui::view_class_description_map;

mod image_ui;
pub(crate) use self::image_ui::show_tensor;
