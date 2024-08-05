use re_log_types::EntityPath;
use re_types::{
    archetypes::{DepthImage, SegmentationImage},
    tensor_data::TensorDataMeaning,
    Archetype,
};

pub fn image_meaning_for_entity(
    entity_path: &EntityPath,
    query: &re_chunk_store::LatestAtQuery,
    store: &re_chunk_store::ChunkStore,
) -> TensorDataMeaning {
    let timeline = &query.timeline();
    if store.entity_has_component_on_timeline(
        timeline,
        entity_path,
        &DepthImage::indicator().name(),
    ) {
        TensorDataMeaning::Depth
    } else if store.entity_has_component_on_timeline(
        timeline,
        entity_path,
        &SegmentationImage::indicator().name(),
    ) {
        TensorDataMeaning::ClassId
    } else {
        TensorDataMeaning::Unknown
    }
}
