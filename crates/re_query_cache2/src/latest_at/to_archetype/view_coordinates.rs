// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/to_archetype.rs

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]

use crate::CachedLatestAtResults;
use re_query2::{PromiseResolver, PromiseResult};
use re_types_core::{Archetype, Loggable as _};
use std::sync::Arc;

impl crate::ToArchetype<re_types::archetypes::ViewCoordinates> for CachedLatestAtResults {
    #[inline]
    fn to_archetype(
        &self,
        resolver: &PromiseResolver,
    ) -> PromiseResult<crate::Result<re_types::archetypes::ViewCoordinates>> {
        re_tracing::profile_function!(<re_types::archetypes::ViewCoordinates>::name());

        // --- Required ---

        use re_types::components::ViewCoordinates;
        let xyz = match self.get_required(<ViewCoordinates>::name()) {
            Ok(xyz) => xyz,
            Err(query_err) => return PromiseResult::Ready(Err(query_err)),
        };
        let xyz = match xyz.to_dense::<ViewCoordinates>(resolver) {
            PromiseResult::Pending => return PromiseResult::Pending,
            PromiseResult::Error(promise_err) => return PromiseResult::Error(promise_err),
            PromiseResult::Ready(query_res) => match query_res {
                Ok(data) => {
                    let Some(first) = data.first().cloned() else {
                        return PromiseResult::Error(std::sync::Arc::new(
                            re_types_core::DeserializationError::missing_data(),
                        ));
                    };
                    first
                }
                Err(query_err) => return PromiseResult::Ready(Err(query_err)),
            },
        };

        // --- Recommended/Optional ---

        // ---

        let arch = re_types::archetypes::ViewCoordinates { xyz };

        PromiseResult::Ready(Ok(arch))
    }
}
