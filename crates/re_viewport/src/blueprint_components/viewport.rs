use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use re_log_types::serde_field::SerdeField;

pub use re_viewer_context::SpaceViewId;

pub const VIEWPORT_PATH: &str = "viewport";

/// Whether a space view is maximized
///
/// ## Example
/// ```
/// # use re_viewport::blueprint_components::SpaceViewMaximized;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     SpaceViewMaximized::data_type(),
///     DataType::Binary
/// );
/// ```
#[derive(Clone, Default, Debug, PartialEq, Eq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(transparent)]
pub struct SpaceViewMaximized(
    #[arrow_field(type = "SerdeField<Option<SpaceViewId>>")] pub Option<SpaceViewId>,
);

re_log_types::arrow2convert_component_shim!(SpaceViewMaximized as "rerun.blueprint.Maximized");

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

#[test]
fn test_maximized_roundtrip() {
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    for data in [
        [SpaceViewMaximized(None)],
        [SpaceViewMaximized(Some(SpaceViewId::random()))],
    ] {
        let array: Box<dyn arrow2::array::Array> = data.try_into_arrow().unwrap();
        let ret: Vec<SpaceViewMaximized> = array.try_into_collection().unwrap();
        assert_eq!(&data, ret.as_slice());
    }
}
