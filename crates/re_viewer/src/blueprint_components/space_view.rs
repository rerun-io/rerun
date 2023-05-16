use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};
use re_data_store::ComponentName;
use re_log_types::{serde_field::SerdeField, Component};

use crate::ui::SpaceView;

/// A [`SpaceView`]
///
/// ## Example
/// ```
/// # use re_viewer::blueprint_components::space_view::SpaceViewComponent;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     SpaceViewComponent::data_type(),
///     DataType::Struct(vec![
///         Field::new("space_view", DataType::Binary, false),
///     ])
/// );
/// ```
#[derive(Clone, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct SpaceViewComponent {
    #[arrow_field(type = "SerdeField<SpaceView>")]
    pub space_view: SpaceView,
}

impl SpaceViewComponent {
    // TODO(jleibs): Can we make this an EntityPath instead?
    pub const SPACEVIEW_PREFIX: &str = "space_view";
}

impl Component for SpaceViewComponent {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.spaceview".into()
    }
}

impl std::fmt::Debug for SpaceViewComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SpaceViewComponent")
    }
}

#[test]
fn test_spaceview() {
    use crate::ui::ViewCategory;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};
    let space_view = SpaceView::new(ViewCategory::Spatial, &"foo".into(), &["foo/bar".into()]);

    let data = [SpaceViewComponent { space_view }];
    let array: Box<dyn arrow2::array::Array> = data.try_into_arrow().unwrap();
    let ret: Vec<SpaceViewComponent> = array.try_into_collection().unwrap();
    assert_eq!(&data, ret.as_slice());
}
