use ahash::HashMap;
use arrow2::{array::TryExtend, datatypes::DataType};
use arrow2_convert::{
    deserialize::ArrowDeserialize, field::ArrowField, serialize::ArrowSerialize, ArrowDeserialize,
    ArrowField, ArrowSerialize,
};

use crate::{
    component_types::{ClassId, KeypointId},
    msg_bundle::Component,
};

use super::{ColorRGBA, Label};

/// Information about an Annotation.
///
/// Can be looked up for a [`ClassId`] or [`KeypointId`].
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct AnnotationInfo {
    /// [`ClassId`] or [`KeypointId`] to which this annotation info belongs.
    pub id: u16,
    pub label: Option<Label>,
    pub color: Option<ColorRGBA>,
}

/// The description of a semantic Class.
///
/// If an entity is annotated with a corresponding [`ClassId`], we should use
/// the attached [`AnnotationInfo`] for labels and colors.
///
/// Keypoints within an annotation class can similarly be annotated with a
/// [`KeypointId`] in which case we should defer to the label and color for the
/// [`AnnotationInfo`] specifically associated with the Keypoint.
///
/// Keypoints within the class can also be decorated with skeletal edges.
/// Keypoint-connections are pairs of [`KeypointId`]s. If an edge is
/// defined, and both keypoints exist within the instance of the class, then the
/// keypoints shold be connected with an edge. The edge should be labeled and
/// colored as described by the class's [`AnnotationInfo`].
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClassDescription {
    pub info: AnnotationInfo,
    pub keypoint_map: HashMap<KeypointId, AnnotationInfo>,

    /// Semantic connections between two keypoints.
    ///
    /// This indicates that an edge line should be drawn between two Keypoints.
    /// Typically used for skeleton edges.
    pub keypoint_connections: Vec<(KeypointId, KeypointId)>,
}

/// Helper struct for converting `ClassDescription` to arrow
#[derive(ArrowField, ArrowSerialize, ArrowDeserialize)]
struct KeypointPairArrow {
    keypoint0: KeypointId,
    keypoint1: KeypointId,
}

/// Helper struct for converting `ClassDescription` to arrow
#[derive(ArrowField, ArrowSerialize, ArrowDeserialize)]
struct ClassDescriptionArrow {
    info: AnnotationInfo,
    keypoint_map: Vec<AnnotationInfo>,
    keypoint_connections: Vec<KeypointPairArrow>,
}

impl From<&ClassDescription> for ClassDescriptionArrow {
    fn from(v: &ClassDescription) -> Self {
        ClassDescriptionArrow {
            info: v.info.clone(),
            keypoint_map: v.keypoint_map.values().cloned().collect(),
            keypoint_connections: v
                .keypoint_connections
                .iter()
                .map(|(k0, k1)| KeypointPairArrow {
                    keypoint0: *k0,
                    keypoint1: *k1,
                })
                .collect(),
        }
    }
}

impl From<ClassDescriptionArrow> for ClassDescription {
    fn from(v: ClassDescriptionArrow) -> Self {
        ClassDescription {
            info: v.info,
            keypoint_map: v
                .keypoint_map
                .into_iter()
                .map(|elem| (KeypointId(elem.id), elem))
                .collect(),
            keypoint_connections: v
                .keypoint_connections
                .into_iter()
                .map(|elem| (elem.keypoint0, elem.keypoint1))
                .collect(),
        }
    }
}

/// The `AnnotationContext` provides additional information on how to display
/// entities.
///
/// Entities can use `ClassId`s and `KeypointId`s to provide annotations, and
/// the labels and colors will be looked up in the appropriate
/// `AnnotationContext`. We use the *first* annotation context we find in the
/// path-hierarchy when searching up through the ancestors of a given entity
/// path.
///
/// ```
/// use re_log_types::component_types::AnnotationContext;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     AnnotationContext::data_type(),
///     DataType::List(Box::new(Field::new(
///         "item",
///         DataType::Struct(vec![
///             Field::new("class_id", DataType::UInt16, false),
///             Field::new(
///                 "class_description",
///                 DataType::Struct(vec![
///                     Field::new(
///                         "info",
///                         DataType::Struct(vec![
///                             Field::new("id", DataType::UInt16, false),
///                             Field::new("label", DataType::Utf8, true),
///                             Field::new("color", DataType::UInt32, true),
///                         ]),
///                         false
///                     ),
///                     Field::new(
///                         "keypoint_map",
///                         DataType::List(Box::new(Field::new(
///                             "item",
///                             DataType::Struct(vec![
///                                 Field::new("id", DataType::UInt16, false),
///                                 Field::new("label", DataType::Utf8, true),
///                                 Field::new("color", DataType::UInt32, true),
///                             ]),
///                             false
///                         ))),
///                         false
///                     ),
///                     Field::new(
///                         "keypoint_connections",
///                         DataType::List(Box::new(Field::new(
///                             "item",
///                             DataType::Struct(vec![
///                                 Field::new("keypoint0", DataType::UInt16, false),
///                                 Field::new("keypoint1", DataType::UInt16, false),
///                             ]),
///                             false
///                         ))),
///                         false
///                     ),
///                 ]),
///                 false
///             )
///         ]),
///         false
///     )))
/// );
/// ```
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnnotationContext {
    pub class_map: HashMap<ClassId, ClassDescription>,
}

impl Component for AnnotationContext {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.annotation_context".into()
    }
}

/// Helper struct for converting `AnnotationContext` to arrow
#[derive(ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct ClassMapElemArrow {
    class_id: ClassId,
    class_description: ClassDescriptionArrow,
}

type AnnotationContextArrow = Vec<ClassMapElemArrow>;

impl From<&AnnotationContext> for AnnotationContextArrow {
    #[inline]
    fn from(v: &AnnotationContext) -> Self {
        v.class_map
            .iter()
            .map(|(class_id, class_description)| ClassMapElemArrow {
                class_id: *class_id,
                class_description: class_description.into(),
            })
            .collect()
    }
}

impl From<Vec<ClassMapElemArrow>> for AnnotationContext {
    #[inline]
    fn from(v: AnnotationContextArrow) -> Self {
        AnnotationContext {
            class_map: v
                .into_iter()
                .map(|elem| (elem.class_id, elem.class_description.into()))
                .collect(),
        }
    }
}

impl ArrowField for AnnotationContext {
    type Type = Self;

    #[inline]
    fn data_type() -> DataType {
        <AnnotationContextArrow as ArrowField>::data_type()
    }
}

impl ArrowSerialize for AnnotationContext {
    type MutableArrayType = <AnnotationContextArrow as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        AnnotationContextArrow::new_array()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        let v: AnnotationContextArrow = v.into();
        array.mut_values().try_extend(v.iter().map(Some))?;
        array.try_push_valid()
    }
}

impl ArrowDeserialize for AnnotationContext {
    type ArrayType = <AnnotationContextArrow as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        let v = <AnnotationContextArrow as ArrowDeserialize>::arrow_deserialize(v);
        v.map(|v| v.into())
    }
}

#[test]
fn test_context_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let context = AnnotationContext {
        class_map: vec![(
            ClassId(13),
            ClassDescription {
                info: AnnotationInfo {
                    id: 32,
                    label: Some(Label("hello".to_owned())),
                    color: Some(ColorRGBA(0x123456)),
                },
                keypoint_map: vec![
                    (
                        KeypointId(43),
                        AnnotationInfo {
                            id: 43,
                            label: Some(Label("head".to_owned())),
                            color: None,
                        },
                    ),
                    (
                        KeypointId(94),
                        AnnotationInfo {
                            id: 94,
                            label: Some(Label("leg".to_owned())),
                            color: Some(ColorRGBA(0x654321)),
                        },
                    ),
                ]
                .into_iter()
                .collect(),
                keypoint_connections: vec![(KeypointId(43), KeypointId(94))].into_iter().collect(),
            },
        )]
        .into_iter()
        .collect(),
    };

    let context_in = vec![context];
    let array: Box<dyn Array> = context_in.try_into_arrow().unwrap();
    let context_out: Vec<AnnotationContext> =
        TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(context_in, context_out);
}
