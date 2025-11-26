use re_log_types::EntityPath;
use re_tf::TransformInfo;
use re_viewer_context::VisualizerExecutionOutput;

use crate::contexts::TransformTreeContext;

/// Retrieves the transform info for the given entity.
pub fn transform_info_for_entity_or_report_error<'a>(
    transform_context: &'a TransformTreeContext,
    entity_path: &EntityPath,
    output: &mut VisualizerExecutionOutput,
) -> Option<&'a TransformInfo> {
    match transform_context.target_from_entity_path(entity_path.hash()) {
        None => {
            output.report_error_for(
                entity_path.clone(),
                "No transform relation known for this entity.",
            );
            None
        }

        Some(Err(re_tf::TransformFromToError::NoPathBetweenFrames { src, target, .. })) => {
            let src = transform_context
                .lookup_frame_id(*src)
                .map_or_else(|| format!("{src:?}"), ToString::to_string);
            let target = transform_context
                .lookup_frame_id(*target)
                .map_or_else(|| format!("{target:?}"), ToString::to_string);
            output.report_error_for(
                entity_path.clone(),
                format!("No transform path from {src} to the view's origin frame ({target})."),
            );
            None
        }

        Some(Err(re_tf::TransformFromToError::UnknownTargetFrame(target))) => {
            // The target frame is the view's origin.
            // This means this could be hit if the view's origin frame doesn't show up in any data.
            let target = transform_context
                .lookup_frame_id(*target)
                .map_or_else(|| format!("{target:?}"), ToString::to_string);
            output.report_error_for(
                entity_path.clone(),
                format!("The view's origin frame {target} is unknown."),
            );
            None
        }

        Some(Err(re_tf::TransformFromToError::UnknownSourceFrame(src))) => {
            // Unclear how we'd hit this. This means that when processing transforms we encountered a coordinate frame that the transform cache didn't know about.
            // That would imply that the cache is lagging behind.
            let src = transform_context
                .lookup_frame_id(*src)
                .map_or_else(|| format!("{src:?}"), ToString::to_string);
            output.report_error_for(
                entity_path.clone(),
                format!("The entity's coordinate frame {src} is unknown."),
            );
            None
        }

        Some(Ok(transform_info)) => Some(transform_info),
    }
}
