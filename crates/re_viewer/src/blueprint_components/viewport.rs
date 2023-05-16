use ahash::HashMap;
use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};
use re_data_store::ComponentName;
use re_log_types::{serde_field::SerdeField, Component};

pub use re_viewer_context::SpaceViewId;

use crate::ui::VisibilitySet;

pub const VIEWPORT_PATH: &str = "viewport";

/// A flag indicating space views should be automatically populated
///
/// ## Example
/// ```
/// # use re_viewer::blueprint_components::viewport::AutoSpaceViews;
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

impl Component for AutoSpaceViews {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.auto_space_views".into()
    }
}

/// The set of currently visible spaces
///
/// ## Example
/// ```
/// # use re_viewer::blueprint_components::viewport::SpaceViewVisibility;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     SpaceViewVisibility::data_type(),
///     DataType::Binary
/// );
/// ```
#[derive(Clone, Default, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(transparent)]
pub struct SpaceViewVisibility(
    #[arrow_field(type = "SerdeField<VisibilitySet>")] pub VisibilitySet,
);

impl Component for SpaceViewVisibility {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.space_view_visibility".into()
    }
}

/// Whether a space view is maximized
///
/// ## Example
/// ```
/// # use re_viewer::blueprint_components::viewport::SpaceViewMaximized;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     SpaceViewMaximized::data_type(),
///     DataType::Binary
/// );
/// ```
#[derive(Clone, Default, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(transparent)]
pub struct SpaceViewMaximized(
    #[arrow_field(type = "Option<SerdeField<SpaceViewId>>")] pub Option<SpaceViewId>,
);

impl Component for SpaceViewMaximized {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.maximized".into()
    }
}

/// The layout of a `Viewport`
///
/// ## Example
/// ```
/// # use re_viewer::blueprint_components::viewport::ViewportLayout;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     ViewportLayout::data_type(),
///     DataType::Struct(vec![
///         Field::new("space_view_keys", DataType::Binary, false),
///         Field::new("trees", DataType::Binary, false),
///         Field::new("has_been_user_edited", DataType::Boolean, false),
///     ])
/// );
/// ```
#[derive(Clone, Default, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct ViewportLayout {
    #[arrow_field(type = "SerdeField<std::collections::BTreeSet<SpaceViewId>>")]
    pub space_view_keys: std::collections::BTreeSet<SpaceViewId>,
    #[arrow_field(type = "SerdeField<HashMap<VisibilitySet, egui_tiles::Tree<SpaceViewId>>>")]
    pub trees: HashMap<VisibilitySet, egui_tiles::Tree<SpaceViewId>>,
    pub has_been_user_edited: bool,
}

impl Component for ViewportLayout {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.viewport_layout".into()
    }
}
