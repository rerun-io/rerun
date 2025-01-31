//! Companion to [`std::ops::ControlFlow`] useful to implement visitor patterns.

use std::ops::ControlFlow;

/// Type to be returned by visitor closure to control the tree traversal flow.
pub enum VisitorControlFlow<B> {
    /// Continue tree traversal
    Continue,

    /// Continue tree traversal but skip the children of the current item.
    SkipBranch,

    /// Stop traversal and return this value.
    Break(B),
}

impl<B> VisitorControlFlow<B> {
    /// Indicates whether we should visit the children of the current node—or entirely stop
    /// traversal.
    ///
    /// Returning a [`ControlFlow`] enables key ergonomics by allowing the use of the short circuit
    /// operator (`?`) while extracting the flag to control traversal of children.
    pub fn visit_children(self) -> ControlFlow<B, bool> {
        match self {
            Self::Break(val) => ControlFlow::Break(val),
            Self::Continue => ControlFlow::Continue(true),
            Self::SkipBranch => ControlFlow::Continue(false),
        }
    }
}
