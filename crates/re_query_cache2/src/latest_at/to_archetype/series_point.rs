// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/to_archetype.rs

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]

use crate::CachedLatestAtResults;
use re_query2::{PromiseResolver, PromiseResult};
use re_types_core::{Archetype, Loggable as _};
use std::sync::Arc;

impl crate::ToArchetype<re_types::archetypes::SeriesPoint> for CachedLatestAtResults {
    #[inline]
    fn to_archetype(
        &self,
        resolver: &PromiseResolver,
    ) -> PromiseResult<crate::Result<re_types::archetypes::SeriesPoint>> {
        re_tracing::profile_function!(<re_types::archetypes::SeriesPoint>::name());

        // --- Required ---

        // --- Recommended/Optional ---

        use re_types::components::Color;
        let color = if let Some(color) = self.get(<Color>::name()) {
            match color.to_dense::<Color>(resolver) {
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

        use re_types::components::MarkerShape;
        let marker = if let Some(marker) = self.get(<MarkerShape>::name()) {
            match marker.to_dense::<MarkerShape>(resolver) {
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

        use re_types::components::Name;
        let name = if let Some(name) = self.get(<Name>::name()) {
            match name.to_dense::<Name>(resolver) {
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

        use re_types::components::MarkerSize;
        let marker_size = if let Some(marker_size) = self.get(<MarkerSize>::name()) {
            match marker_size.to_dense::<MarkerSize>(resolver) {
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

        let arch = re_types::archetypes::SeriesPoint {
            color,
            marker,
            name,
            marker_size,
        };

        PromiseResult::Ready(Ok(arch))
    }
}
