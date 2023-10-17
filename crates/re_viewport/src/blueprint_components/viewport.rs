use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use re_log_types::serde_field::SerdeField;

pub use re_viewer_context::SpaceViewId;

pub const VIEWPORT_PATH: &str = "viewport";

/// The layout of a `Viewport`
///
/// ## Example
/// ```
/// # use re_viewport::blueprint_components::ViewportLayout;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     ViewportLayout::data_type(),
///     DataType::Struct(vec![
///         Field::new("space_view_keys", DataType::Binary, false),
///         Field::new("tree", DataType::Binary, false),
///         Field::new("auto_layout", DataType::Boolean, false),
///     ])
/// );
/// ```
#[derive(Clone, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct ViewportLayout {
    #[arrow_field(type = "SerdeField<std::collections::BTreeSet<SpaceViewId>>")]
    pub space_view_keys: std::collections::BTreeSet<SpaceViewId>,

    #[arrow_field(type = "SerdeField<egui_tiles::Tree<SpaceViewId>>")]
    pub tree: egui_tiles::Tree<SpaceViewId>,

    pub auto_layout: bool,
}

impl Default for ViewportLayout {
    fn default() -> Self {
        Self {
            space_view_keys: Default::default(),
            tree: Default::default(),
            auto_layout: true,
        }
    }
}

re_log_types::arrow2convert_component_shim!(ViewportLayout as "rerun.blueprint.ViewportLayout");
