// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/to_archetype.rs

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]

use crate::CachedLatestAtResults;
use re_query2::{PromiseResolver, PromiseResult};
use re_types_core::{Archetype, Loggable as _};
use std::sync::Arc;

impl crate::ToArchetype<re_types_blueprint::blueprint::archetypes::ViewportBlueprint>
    for CachedLatestAtResults
{
    #[inline]
    fn to_archetype(
        &self,
        resolver: &PromiseResolver,
    ) -> PromiseResult<crate::Result<re_types_blueprint::blueprint::archetypes::ViewportBlueprint>>
    {
        re_tracing::profile_function!(
            <re_types_blueprint::blueprint::archetypes::ViewportBlueprint>::name()
        );

        // --- Required ---

        // --- Recommended/Optional ---

        use re_types_blueprint::blueprint::components::RootContainer;
        let root_container = if let Some(root_container) = self.get(<RootContainer>::name()) {
            match root_container.to_dense::<RootContainer>(resolver) {
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

        use re_types_blueprint::blueprint::components::SpaceViewMaximized;
        let maximized = if let Some(maximized) = self.get(<SpaceViewMaximized>::name()) {
            match maximized.to_dense::<SpaceViewMaximized>(resolver) {
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

        use re_types_blueprint::blueprint::components::AutoLayout;
        let auto_layout = if let Some(auto_layout) = self.get(<AutoLayout>::name()) {
            match auto_layout.to_dense::<AutoLayout>(resolver) {
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

        use re_types_blueprint::blueprint::components::AutoSpaceViews;
        let auto_space_views = if let Some(auto_space_views) = self.get(<AutoSpaceViews>::name()) {
            match auto_space_views.to_dense::<AutoSpaceViews>(resolver) {
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

        use re_types::blueprint::components::ViewerRecommendationHash;
        let past_viewer_recommendations = if let Some(past_viewer_recommendations) =
            self.get(<ViewerRecommendationHash>::name())
        {
            match past_viewer_recommendations.to_dense::<ViewerRecommendationHash>(resolver) {
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

        let arch = re_types_blueprint::blueprint::archetypes::ViewportBlueprint {
            root_container,
            maximized,
            auto_layout,
            auto_space_views,
            past_viewer_recommendations,
        };

        PromiseResult::Ready(Ok(arch))
    }
}
