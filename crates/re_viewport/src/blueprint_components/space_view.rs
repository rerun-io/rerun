use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use re_log_types::serde_field::SerdeField;

use crate::space_view::SpaceViewBlueprint;

/// A [`SpaceViewBlueprint`]
///
/// ## Example
/// ```
/// # use re_viewport::blueprint_components::SpaceViewComponent;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     SpaceViewComponent::data_type(),
///     DataType::Struct(vec![
///         Field::new("space_view", DataType::Binary, false),
///     ])
/// );
/// ```
#[derive(Clone, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct SpaceViewComponent {
    #[arrow_field(type = "SerdeField<SpaceViewBlueprint>")]
    pub space_view: SpaceViewBlueprint,
}

impl SpaceViewComponent {
    // TODO(jleibs): Can we make this an EntityPath instead?
    pub const SPACEVIEW_PREFIX: &str = "space_view";
}

impl std::fmt::Debug for SpaceViewComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SpaceViewComponent")
    }
}

re_log_types::arrow2convert_component_shim!(SpaceViewComponent as "rerun.blueprint.SpaceView");

#[test]
fn test_spaceview() {
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let space_view = SpaceViewBlueprint::new(
        "Spatial".into(),
        &"foo".into(),
        std::iter::once(&"foo/bar".into()),
    );

    let data = [SpaceViewComponent { space_view }];
    let array: Box<dyn arrow2::array::Array> = data.try_into_arrow().unwrap();
    let ret: Vec<SpaceViewComponent> = array.try_into_collection().unwrap();
    assert_eq!(data.len(), ret.len());
    assert!(data
        .iter()
        .zip(ret)
        .all(|(l, r)| !l.space_view.has_edits(&r.space_view)));
}
