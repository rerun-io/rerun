// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/to_archetype.rs

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]

use crate::{LatestAtResults, PromiseResolver, PromiseResult};
use re_types_core::{Archetype, Loggable as _};
use std::sync::Arc;

impl crate::ToArchetype<re_types::archetypes::Boxes2D> for LatestAtResults {
    #[inline]
    fn to_archetype(
        &self,
        resolver: &PromiseResolver,
    ) -> PromiseResult<crate::Result<re_types::archetypes::Boxes2D>> {
        re_tracing::profile_function!(<re_types::archetypes::Boxes2D>::name());

        // --- Required ---

        use re_types::components::HalfSizes2D;
        let half_sizes = match self.get_required(<HalfSizes2D>::name()) {
            Ok(half_sizes) => half_sizes,
            Err(query_err) => return PromiseResult::Ready(Err(query_err)),
        };
        let half_sizes = match half_sizes.to_dense::<HalfSizes2D>(resolver) {
            PromiseResult::Pending => return PromiseResult::Pending,
            PromiseResult::Error(promise_err) => return PromiseResult::Error(promise_err),
            PromiseResult::Ready(query_res) => match query_res {
                Ok(data) => data.to_vec(),
                Err(query_err) => return PromiseResult::Ready(Err(query_err)),
            },
        };

        // --- Recommended/Optional ---

        use re_types::components::Position2D;
        let centers = if let Some(centers) = self.get(<Position2D>::name()) {
            match centers.to_dense::<Position2D>(resolver) {
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(promise_err) => return PromiseResult::Error(promise_err),
                PromiseResult::Ready(query_res) => match query_res {
                    Ok(data) => Some(data.to_vec()),
                    Err(query_err) => return PromiseResult::Ready(Err(query_err)),
                },
            }
        } else {
            None
        };

        use re_types::components::Color;
        let colors = if let Some(colors) = self.get(<Color>::name()) {
            match colors.to_dense::<Color>(resolver) {
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(promise_err) => return PromiseResult::Error(promise_err),
                PromiseResult::Ready(query_res) => match query_res {
                    Ok(data) => Some(data.to_vec()),
                    Err(query_err) => return PromiseResult::Ready(Err(query_err)),
                },
            }
        } else {
            None
        };

        use re_types::components::Radius;
        let radii = if let Some(radii) = self.get(<Radius>::name()) {
            match radii.to_dense::<Radius>(resolver) {
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(promise_err) => return PromiseResult::Error(promise_err),
                PromiseResult::Ready(query_res) => match query_res {
                    Ok(data) => Some(data.to_vec()),
                    Err(query_err) => return PromiseResult::Ready(Err(query_err)),
                },
            }
        } else {
            None
        };

        use re_types::components::Text;
        let labels = if let Some(labels) = self.get(<Text>::name()) {
            match labels.to_dense::<Text>(resolver) {
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(promise_err) => return PromiseResult::Error(promise_err),
                PromiseResult::Ready(query_res) => match query_res {
                    Ok(data) => Some(data.to_vec()),
                    Err(query_err) => return PromiseResult::Ready(Err(query_err)),
                },
            }
        } else {
            None
        };

        use re_types::components::DrawOrder;
        let draw_order = if let Some(draw_order) = self.get(<DrawOrder>::name()) {
            match draw_order.to_dense::<DrawOrder>(resolver) {
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

        use re_types::components::ClassId;
        let class_ids = if let Some(class_ids) = self.get(<ClassId>::name()) {
            match class_ids.to_dense::<ClassId>(resolver) {
                PromiseResult::Pending => return PromiseResult::Pending,
                PromiseResult::Error(promise_err) => return PromiseResult::Error(promise_err),
                PromiseResult::Ready(query_res) => match query_res {
                    Ok(data) => Some(data.to_vec()),
                    Err(query_err) => return PromiseResult::Ready(Err(query_err)),
                },
            }
        } else {
            None
        };

        // ---

        let arch = re_types::archetypes::Boxes2D {
            half_sizes,
            centers,
            colors,
            radii,
            labels,
            draw_order,
            class_ids,
        };

        PromiseResult::Ready(Ok(arch))
    }
}
