mod scene;
pub use self::scene::{Box2D, Image, LineSegments2D, ObjectPaintProperties, Point2D, Scene2D};

mod ui;
pub(crate) use self::ui::{view_2d, State2D};

mod class_description_ui;
pub(crate) use self::class_description_ui::view_class_description_map;

mod legend;
pub use self::legend::{
    ClassDescription, ClassDescriptionMap, ColorMapping, LabelMapping, Legend, Legends,
};

mod image_ui;
pub(crate) use self::image_ui::{show_tensor, show_zoomed_image_region_tooltip};
