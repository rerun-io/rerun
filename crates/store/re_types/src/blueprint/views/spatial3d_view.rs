// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/views/spatial3d.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::external::arrow;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **View**: For viewing spatial 3D data.
#[derive(Clone, Debug)]
pub struct Spatial3DView {
    /// Configuration for the background of the view.
    pub background: crate::blueprint::archetypes::Background,

    /// Configuration for the 3D line grid.
    pub line_grid: crate::blueprint::archetypes::LineGrid3D,

    /// Configures which range on each timeline is shown by this view (unless specified differently per entity).
    ///
    /// If not specified, the default is to show the latest state of each component.
    /// If a timeline is specified more than once, the first entry will be used.
    pub time_ranges: crate::blueprint::archetypes::VisibleTimeRanges,
}

impl ::re_types_core::View for Spatial3DView {
    #[inline]
    fn identifier() -> ::re_types_core::ViewClassIdentifier {
        "3D".into()
    }
}

impl ::re_byte_size::SizeBytes for Spatial3DView {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.background.heap_size_bytes()
            + self.line_grid.heap_size_bytes()
            + self.time_ranges.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::blueprint::archetypes::Background>::is_pod()
            && <crate::blueprint::archetypes::LineGrid3D>::is_pod()
            && <crate::blueprint::archetypes::VisibleTimeRanges>::is_pod()
    }
}
