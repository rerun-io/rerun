use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};
use re_data_store::ComponentName;
use re_log_types::{serde_field::SerdeField, Component};

use crate::ui::SpaceView;

#[derive(Clone, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct SpaceViewComponent {
    #[arrow_field(type = "SerdeField<SpaceView>")]
    pub space_view: SpaceView,
}

impl Component for SpaceViewComponent {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.spaceview".into()
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
    assert_eq!(
        data[0].space_view.space_path,
        ret.as_slice()[0].space_view.space_path
    );
}
