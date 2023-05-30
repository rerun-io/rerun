use super::Vec3D;
use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

/// A 3D Arrow
///
/// ## Examples
///
/// ```
/// use re_log_types::component_types::Arrow3D;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     Arrow3D::data_type(),
///     DataType::Struct(vec![
///         Field::new("origin",
///                    DataType::FixedSizeList(
///                        Box::new(Field::new("item", DataType::Float32, false)),
///                        3
///                    ),
///                    false),
///         Field::new("vector",
///                    DataType::FixedSizeList(
///                        Box::new(Field::new("item", DataType::Float32, false)),
///                        3
///                    ),
///                    false),
///     ])
/// );
/// ```
#[derive(Copy, Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Arrow3D {
    pub origin: Vec3D,
    pub vector: Vec3D,
}

impl Component for Arrow3D {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.arrow3d".into()
    }
}

#[test]
fn test_arrow3d_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let arrows_in = vec![
        Arrow3D {
            origin: [1.0, 2.0, 3.0].into(),
            vector: [4.0, 5.0, 6.0].into(),
        },
        Arrow3D {
            origin: [11.0, 12.0, 13.0].into(),
            vector: [14.0, 15.0, 16.0].into(),
        },
    ];
    let array: Box<dyn Array> = arrows_in.try_into_arrow().unwrap();
    let arrows_out: Vec<Arrow3D> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(arrows_in, arrows_out);
}
