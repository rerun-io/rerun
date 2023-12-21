use re_log_types::EntityPath;
use re_types::{
    archetypes::{DepthImage, SegmentationImage},
    tensor_data::TensorDataMeaning,
    Archetype,
};
use re_viewer_context::ViewerContext;

pub fn image_meaning_for_entity(
    entity_path: &EntityPath,
    ctx: &ViewerContext<'_>,
) -> TensorDataMeaning {
    let store = ctx.entity_db.store();
    let timeline = &ctx.current_query_for_entity_path(entity_path).timeline;
    if store.entity_has_component(timeline, entity_path, &DepthImage::indicator().name()) {
        TensorDataMeaning::Depth
    } else if store.entity_has_component(
        timeline,
        entity_path,
        &SegmentationImage::indicator().name(),
    ) {
        TensorDataMeaning::ClassId
    } else {
        TensorDataMeaning::Unknown
    }
}
