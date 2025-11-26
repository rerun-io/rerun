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

        Some(Err(re_tf::TransformFromToError::NoPathBetweenFrames { .. })) => {
            // TODO(RR-2997): Pretty print out the frames involved.
            output.report_error_for(
                entity_path.clone(),
                "No transform path to the view's origin frame.",
            );
            None
        }

        Some(Err(re_tf::TransformFromToError::UnknownTargetFrame { .. })) => {
            // The target frame is the view's origin.
            // This means this could be hit if the view's origin frame doesn't show up in any data.
            // TODO(RR-2997): Pretty print out the frames involved.
            output.report_error_for(entity_path.clone(), "The view's origin frame is unknown.");
            None
        }

        Some(Err(re_tf::TransformFromToError::UnknownSourceFrame { .. })) => {
            // Unclear how we'd hit this. This means that when processing transforms we encountered a coordinate frame that the transform cache didn't know about.
            // That would imply that the cache is lagging behind.
            // TODO(RR-2997): Pretty print out the frames involved.
            output.report_error_for(
                entity_path.clone(),
                "The entity's coordinate frame is unknown.",
            );
            None
        }

        Some(Ok(transform_info)) => Some(transform_info),
    }
}
