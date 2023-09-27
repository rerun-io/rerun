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
    let store = &ctx.store_db.entity_db.data_store;
    let timeline = &ctx.current_query().timeline;
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
