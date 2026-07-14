use glam::DAffine3;

use crate::TransformFrameIdHash;

/// A transform from a child frame to a parent frame.
#[derive(Clone, Debug, PartialEq, re_byte_size::SizeBytes)]
pub struct ParentFromChildTransform {
    /// The frame we're transforming into.
    pub parent: TransformFrameIdHash,

    /// The transform from the child frame to the parent frame.
    pub transform: DAffine3,
}
