use re_arrow_store::LatestAtQuery;
use re_data_store::{query_latest_single, EntityPath};
use re_log_types::Transform;
use re_viewer_context::ViewerContext;

/// Find closest entity with a pinhole transform.
pub fn closest_pinhole_transform(
    ctx: &ViewerContext<'_>,
    entity_path: &EntityPath,
    query: &LatestAtQuery,
) -> Option<EntityPath> {
    crate::profile_function!();

    let mut pinhole_ent_path = None;
    let mut cur_path = Some(entity_path.clone());
    while let Some(path) = cur_path {
        if let Some(Transform::Pinhole(_)) =
            query_latest_single::<Transform>(&ctx.log_db.entity_db, &path, query)
        {
            pinhole_ent_path = Some(path);
            break;
        }
        cur_path = path.parent();
    }
    pinhole_ent_path
}
