use std::{collections::BTreeMap, sync::Arc};

/// An 16-bit ID representating a type of semantic class.
///
/// Used to look up a [`ClassDescription`] within the [`AnnotationContext`].
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClassId(pub u16);

/// An 16-bit ID representating a type of semantic keypoint within a class.
///
/// `KeypointId`s are only meaningful within the context of a [`ClassDescription`].
///
/// Used to look up an [`AnnotationInfo`] for a Keypoint within the [`AnnotationContext`].
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeypointId(pub u16);

/// A semantic skeleton edge between two keypoints.
///
/// This indicates that an edge line should be drawn between two Keypoints.
///
/// `KeypointSkeletonEdges` are only meaningful within the context of a [`ClassDescription`].
///
/// Used to look up an [`AnnotationInfo`] for a `KeypointSkeletonEdge` within
/// the [`AnnotationContext`].
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeypointSkeletonEdge(pub KeypointId, pub KeypointId);

/// Information about an Annotation.
///
/// Can be looked up for a [`ClassId`], [`KeypointId`], or [`KeypointSkeletonEdge`]h
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnnotationInfo {
    pub label: Option<Arc<String>>,
    pub color: Option<[u8; 4]>,
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
/// Keypoints within the class can also be decorated with skeletal edges. The
/// [`KeypointSkeletonEdge`] is simply a pair of [`KeypointId`]s. If an edge is
/// defined, and both keypoints exist within the instance of the class, then the
/// keypoints shold be connected with an edge. The edge should be labeled and
/// colored as described by the [`AnnotationInfo`].
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClassDescription {
    pub info: AnnotationInfo,
    pub keypoint_map: BTreeMap<KeypointId, AnnotationInfo>,
    pub skeleton_edges: BTreeMap<KeypointSkeletonEdge, AnnotationInfo>,
}

/// The AnnotationContext provides aditional information on how to display
/// entities.
///
/// Entities can use `ClassId`s and `KeypointId`s to provide annotations, and
/// the labels and colors will be looked up in the appropriate
/// `AnnotationContext`. We use the *first* annotation context we find in the
/// path-hierarchy when searching up through the ancestors of a given object
/// path.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnnotationContext {
    pub class_map: BTreeMap<ClassId, ClassDescription>,
}
