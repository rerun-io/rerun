// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/to_archetype.rs

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]

use crate::CachedLatestAtResults;
use re_query2::{PromiseResolver, PromiseResult};
use re_types_core::{Archetype, Loggable as _};
use std::sync::Arc;

impl crate::ToArchetype<re_types::archetypes::Arrows3D> for CachedLatestAtResults {
    #[inline]
    fn to_archetype(
        &self,
        resolver: &PromiseResolver,
    ) -> PromiseResult<re_types::archetypes::Arrows3D> {
        re_tracing::profile_function!(<re_types::archetypes::Arrows3D>::name());

        // --- Required ---

        use re_types::components::Vector3D;
        let vectors = match self.get_required(<Vector3D>::name()) {
            Ok(vectors) => vectors,
            Err(err) => return PromiseResult::Error(Arc::new(err)),
        };
        let vectors = match vectors.to_dense::<Vector3D>(resolver).flatten() {
            PromiseResult::Ready(data) => data.to_vec(),
            PromiseResult::Pending => return PromiseResult::Pending,
            PromiseResult::Error(err) => return PromiseResult::Error(err),
        };

        // --- Recommended/Optional ---

        use re_types::components::Position3D;
        let origins = if let Some(origins) = self.get(<Position3D>::name()) {
            match origins.to_dense::<Position3D>(resolver).flatten() {
                PromiseResult::Ready(data) => Some(data.to_vec()),
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(err) => return PromiseResult::Error(err),
            }
        } else {
            None
        };

        use re_types::components::Radius;
        let radii = if let Some(radii) = self.get(<Radius>::name()) {
            match radii.to_dense::<Radius>(resolver).flatten() {
                PromiseResult::Ready(data) => Some(data.to_vec()),
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(err) => return PromiseResult::Error(err),
            }
        } else {
            None
        };

        use re_types::components::Color;
        let colors = if let Some(colors) = self.get(<Color>::name()) {
            match colors.to_dense::<Color>(resolver).flatten() {
                PromiseResult::Ready(data) => Some(data.to_vec()),
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(err) => return PromiseResult::Error(err),
            }
        } else {
            None
        };

        use re_types::components::Text;
        let labels = if let Some(labels) = self.get(<Text>::name()) {
            match labels.to_dense::<Text>(resolver).flatten() {
                PromiseResult::Ready(data) => Some(data.to_vec()),
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(err) => return PromiseResult::Error(err),
            }
        } else {
            None
        };

        use re_types::components::ClassId;
        let class_ids = if let Some(class_ids) = self.get(<ClassId>::name()) {
            match class_ids.to_dense::<ClassId>(resolver).flatten() {
                PromiseResult::Ready(data) => Some(data.to_vec()),
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(err) => return PromiseResult::Error(err),
            }
        } else {
            None
        };

        // ---

        let arch = re_types::archetypes::Arrows3D {
            vectors,
            origins,
            radii,
            colors,
            labels,
            class_ids,
        };

        PromiseResult::Ready(arch)
    }
}
