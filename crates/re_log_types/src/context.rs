use std::sync::Arc;

use ahash::HashMap;

/// An 16-bit ID representing a type of semantic class.
///
/// Used to look up a [`ClassDescription`] within the [`AnnotationContext`].
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassId(pub u16);

/// An 16-bit ID representing a type of semantic keypoint within a class.
///
/// `KeypointId`s are only meaningful within the context of a [`ClassDescription`].
///
/// Used to look up an [`AnnotationInfo`] for a Keypoint within the [`AnnotationContext`].
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeypointId(pub u16);

/// Information about an Annotation.
///
/// Can be looked up for a [`ClassId`], [`KeypointId`], or [`KeypointSkeletonEdge`]h
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnnotationInfo {
    /// [`ClassId`] or [`KeypointId`] to which this annotation info belongs.
    pub id: u16,
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
    pub keypoint_map: HashMap<KeypointId, AnnotationInfo>,

    /// Semantic connections between two keypoints.
    ///
    /// This indicates that an edge line should be drawn between two Keypoints.
    /// Typically used for skeleton edges.
    pub keypoint_connections: Vec<(KeypointId, KeypointId)>,
}

/// The `AnnotationContext` provides aditional information on how to display
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
    pub class_map: HashMap<ClassId, ClassDescription>,
}
