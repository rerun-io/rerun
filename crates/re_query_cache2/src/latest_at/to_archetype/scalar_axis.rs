// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/to_archetype.rs

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]

use crate::CachedLatestAtResults;
use re_query2::{PromiseResolver, PromiseResult};
use re_types_core::{Archetype, Loggable as _};
use std::sync::Arc;

impl crate::ToArchetype<re_types::blueprint::archetypes::ScalarAxis> for CachedLatestAtResults {
    #[inline]
    fn to_archetype(
        &self,
        resolver: &PromiseResolver,
    ) -> PromiseResult<crate::Result<re_types::blueprint::archetypes::ScalarAxis>> {
        re_tracing::profile_function!(<re_types::blueprint::archetypes::ScalarAxis>::name());

        // --- Required ---

        // --- Recommended/Optional ---

        use re_types::components::Range1D;
        let range = if let Some(range) = self.get(<Range1D>::name()) {
            match range.to_dense::<Range1D>(resolver) {
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(promise_err) => return PromiseResult::Error(promise_err),
                PromiseResult::Ready(query_res) => match query_res {
                    Ok(data) => data.first().cloned(),
                    Err(query_err) => return PromiseResult::Ready(Err(query_err)),
                },
            }
        } else {
            None
        };

        use re_types::blueprint::components::LockRangeDuringZoom;
        let lock_range_during_zoom =
            if let Some(lock_range_during_zoom) = self.get(<LockRangeDuringZoom>::name()) {
                match lock_range_during_zoom.to_dense::<LockRangeDuringZoom>(resolver) {
                    PromiseResult::Pending => return PromiseResult::Pending,
                    PromiseResult::Error(promise_err) => return PromiseResult::Error(promise_err),
                    PromiseResult::Ready(query_res) => match query_res {
                        Ok(data) => data.first().cloned(),
                        Err(query_err) => return PromiseResult::Ready(Err(query_err)),
                    },
                }
            } else {
                None
            };

        // ---

        let arch = re_types::blueprint::archetypes::ScalarAxis {
            range,
            lock_range_during_zoom,
        };

        PromiseResult::Ready(Ok(arch))
    }
}
