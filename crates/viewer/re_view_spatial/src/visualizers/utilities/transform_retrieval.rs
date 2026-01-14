use re_log_types::EntityPath;
use re_sdk_types::ViewClassIdentifier;
use re_viewer_context::{ViewClass as _, VisualizerExecutionOutput};

use crate::contexts::{TransformInfo, TransformTreeContext};
use crate::view_kind::SpatialViewKind;

/// Derive the spatial view kind from the view class identifier.
pub fn spatial_view_kind_from_view_class(class: ViewClassIdentifier) -> SpatialViewKind {
    if class == crate::SpatialView3D::identifier() {
        SpatialViewKind::ThreeD
    } else if class == crate::SpatialView2D::identifier() {
        SpatialViewKind::TwoD
    } else {
        debug_assert!(false, "Not a spatial view class identifier {class:?}");
        SpatialViewKind::TwoD
    }
}

/// Retrieves the transform info for the given entity and checks if it is valid for the archetype's space kind.
pub fn transform_info_for_archetype_or_report_error<'a>(
    entity_path: &EntityPath,
    transform_context: &'a TransformTreeContext,
    archetype_kind: Option<SpatialViewKind>,
    view_kind: SpatialViewKind,
    output: &mut VisualizerExecutionOutput,
) -> Option<&'a TransformInfo> {
    let result = transform_context.target_from_entity_path(entity_path.hash());
    let transform_info = match format_transform_info_result(transform_context, result) {
        Ok(transform_info) => transform_info,
        Err(err_msg) => {
            output.report_error_for(entity_path.clone(), err_msg);
            return None;
        }
    };

    is_valid_space_for_content(
        entity_path,
        transform_context,
        transform_info,
        archetype_kind,
        view_kind,
        output,
    )
    .then_some(transform_info)
}

/// Formats the result of a transform retrieval into a user-friendly message.
pub fn format_transform_info_result<'a>(
    transform_context: &TransformTreeContext,
    result: Option<&'a Result<TransformInfo, re_tf::TransformFromToError>>,
) -> Result<&'a TransformInfo, String> {
    match result {
        None => Err("No transform relation known for this entity.".to_owned()),

        Some(Err(re_tf::TransformFromToError::NoPathBetweenFrames { src, target, .. })) => {
            let src = transform_context.format_frame(*src);
            let target = transform_context.format_frame(*target);
            Err(format!(
                "No transform path from {src:?} to the view's origin frame ({target:?})."
            ))
        }

        Some(Err(re_tf::TransformFromToError::UnknownTargetFrame(target))) => {
            // The target frame is the view's origin.
            // This means this could be hit if the view's origin frame doesn't show up in any data.
            let target = transform_context.format_frame(*target);
            Err(format!("The view's origin frame {target:?} is unknown."))
        }

        Some(Err(re_tf::TransformFromToError::UnknownSourceFrame(src))) => {
            // Unclear how we'd hit this. This means that when processing transforms we encountered a coordinate frame that the transform cache didn't know about.
            // That would imply that the cache is lagging behind.
            let src = transform_context.format_frame(*src);
            Err(format!("The entity's coordinate frame {src:?} is unknown."))
        }

        Some(Ok(transform_info)) => Ok(transform_info),
    }
}

pub fn is_valid_space_for_content(
    entity_path: &EntityPath,
    transform_context: &TransformTreeContext,
    transform: &TransformInfo,
    content_kind: Option<SpatialViewKind>,
    view_kind: SpatialViewKind,
    output: &mut VisualizerExecutionOutput,
) -> bool {
    let Some(content_view_kind) = content_kind else {
        // This means the content doesn't have any particular view kind affinity, we expect it to be handled elsewhere if at all.
        return true;
    };

    // Keep in mind that even if this is `Some`, this is not the necessarily the same as the space origin (== target frame of the view),
    // but may be an ancestor of it.
    let target_frame_pinhole_root = transform_context.target_frame_pinhole_root();

    // General failure case for 3D views: if we're in a 3D view, but the origin is under a pinhole, things get really weird!
    //
    // Everything in this 3D view is technically 2D already, but we still have the 3D controls etc.
    // (We can however, still show some "agnostic" content like the Pinhole itself)
    if view_kind == SpatialViewKind::ThreeD
        && let Some(target_frame_pinhole_root) = target_frame_pinhole_root
    {
        let origin = transform_context.format_frame(target_frame_pinhole_root);
        output.report_error_for(
            entity_path.clone(),
            format!("The origin of the 3D view ({origin:?}) is under pinhole projection which is not supported by most 3D visualizations."),
        );
        return false;
    }

    let transform_has_pinhole_ancestor = transform_context
        .pinhole_tree_root_info(transform.tree_root())
        .is_some();

    match content_view_kind {
        SpatialViewKind::TwoD => {
            match view_kind {
                SpatialViewKind::TwoD => {
                    // Degenerated case: 2D content is under a pinhole which itself is NOT the pinhole that the 2D view is in.
                    // We don't allow this since this would mean to apply a 3D->2D projection to a space that's already 2D.
                    if transform_has_pinhole_ancestor
                        && target_frame_pinhole_root.is_none_or(|target_frame_pinhole_root| {
                            target_frame_pinhole_root != transform.tree_root()
                        })
                    {
                        output.report_error_for(
                            entity_path.clone(),
                            "Can't visualize 2D content with a pinhole ancestor that's embedded within the 2D view. This applies a 3D → 2D projection to a space that's already regarded 2D.",
                        );
                        false
                    } else {
                        true
                    }
                }

                SpatialViewKind::ThreeD => {
                    // 2D content in a 3D view needs to be under a Pinhole transform.
                    if transform_has_pinhole_ancestor {
                        true
                    } else {
                        output.report_error_for(
                            entity_path.clone(),
                            "2D visualizers require a pinhole ancestor to be shown in a 3D view.",
                        );
                        false
                    }
                }
            }
        }

        SpatialViewKind::ThreeD => {
            // View agnostic failure case for 3D content: if the 3D content is under a pinhole projection, we can't show it!
            if transform_has_pinhole_ancestor {
                output.report_error_for(
                    entity_path.clone(),
                    "Can't visualize 3D content that is under a pinhole projection.",
                );
                return false;
            }

            match view_kind {
                SpatialViewKind::TwoD => {
                    // 3D content in 2D works only if there's a Pinhole transform at the origin of the view.
                    //
                    // TODO(andreas): What's actually keeping us from allowing the 2D view to be rooted _under_ a pinhole, e.g. `/pinhole_here/some_2d_stuff`?
                    // Should still work transform-wise, but the 2D view's implementation is not supporting this right now.
                    //
                    // Note that this means nothing in this visualizer can actually run, but it's easier to keep
                    // this check here than to fail the entire visualizer.
                    if target_frame_pinhole_root == Some(transform_context.target_frame()) {
                        true
                    } else {
                        output.report_error_for(
                            entity_path.clone(),
                            "3D visualizers require a pinhole at the origin of the 2D view.",
                        );
                        false
                    }
                }

                SpatialViewKind::ThreeD => true, // Valid 3D content in a valid 3D view is always fine.
            }
        }
    }
}
