use glam::DAffine3;
use re_byte_size::SizeBytes;

use crate::TransformFrameIdHash;

/// A transform from a child frame to a parent frame.
#[derive(Clone, Debug, PartialEq)]
pub struct ParentFromChildTransform {
    /// The frame we're transforming into.
    pub parent: TransformFrameIdHash,

    /// The transform from the child frame to the parent frame.
    pub transform: DAffine3,
}

impl SizeBytes for ParentFromChildTransform {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self { parent, transform } = self;

        parent.heap_size_bytes() + transform.heap_size_bytes()
    }
}
