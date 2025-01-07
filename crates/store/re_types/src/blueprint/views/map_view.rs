// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/views/map.fbs".

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

/// **View**: A 2D map view to display geospatial primitives.
#[derive(Clone, Debug)]
pub struct MapView {
    /// Configures the zoom level of the map view.
    pub zoom: crate::blueprint::archetypes::MapZoom,

    /// Configuration for the background map of the map view.
    pub background: crate::blueprint::archetypes::MapBackground,
}

impl ::re_types_core::View for MapView {
    #[inline]
    fn identifier() -> ::re_types_core::ViewClassIdentifier {
        "Map".into()
    }
}

impl ::re_byte_size::SizeBytes for MapView {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.zoom.heap_size_bytes() + self.background.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::blueprint::archetypes::MapZoom>::is_pod()
            && <crate::blueprint::archetypes::MapBackground>::is_pod()
    }
}
