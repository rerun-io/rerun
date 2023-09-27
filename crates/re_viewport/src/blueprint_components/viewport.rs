use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use re_log_types::{serde_field::SerdeField, ComponentName, LegacyComponent};

pub use re_viewer_context::SpaceViewId;

pub const VIEWPORT_PATH: &str = "viewport";

/// A flag indicating space views should be automatically populated
///
/// ## Example
/// ```
/// # use re_viewport::blueprint_components::AutoSpaceViews;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     AutoSpaceViews::data_type(),
///     DataType::Boolean
/// );
/// ```
#[derive(Clone, Default, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(transparent)]
pub struct AutoSpaceViews(pub bool);

impl LegacyComponent for AutoSpaceViews {
    #[inline]
    fn legacy_name() -> ComponentName {
        "rerun.blueprint.auto_space_views".into()
    }
}

re_log_types::component_legacy_shim!(AutoSpaceViews);

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

impl LegacyComponent for SpaceViewMaximized {
    #[inline]
    fn legacy_name() -> ComponentName {
        "rerun.blueprint.maximized".into()
    }
}

re_log_types::component_legacy_shim!(SpaceViewMaximized);

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
#[derive(Clone, Default, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct ViewportLayout {
    #[arrow_field(type = "SerdeField<std::collections::BTreeSet<SpaceViewId>>")]
    pub space_view_keys: std::collections::BTreeSet<SpaceViewId>,

    #[arrow_field(type = "SerdeField<egui_tiles::Tree<SpaceViewId>>")]
    pub tree: egui_tiles::Tree<SpaceViewId>,

    pub auto_layout: bool,
}

impl LegacyComponent for ViewportLayout {
    #[inline]
    fn legacy_name() -> ComponentName {
        "rerun.blueprint.viewport_layout".into()
    }
}

re_log_types::component_legacy_shim!(ViewportLayout);

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
