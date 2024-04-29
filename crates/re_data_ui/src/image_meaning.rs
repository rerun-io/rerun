use re_entity_db::EntityDb;
use re_log_types::EntityPath;
use re_types::{
    archetypes::{DepthImage, SegmentationImage},
    tensor_data::TensorDataMeaning,
    Archetype,
};

pub fn image_meaning_for_entity(
    entity_path: &EntityPath,
    query: &re_query::LatestAtQuery,
    db: &EntityDb,
) -> TensorDataMeaning {
    let timeline = &query.timeline();
    if db.query_caches().entity_has_component(
        db.store(),
        timeline,
        entity_path,
        &DepthImage::indicator().name(),
    ) {
        TensorDataMeaning::Depth
    } else if db.query_caches().entity_has_component(
        db.store(),
        timeline,
        entity_path,
        &SegmentationImage::indicator().name(),
    ) {
        TensorDataMeaning::ClassId
    } else {
        TensorDataMeaning::Unknown
    }
}
