use std::collections::BTreeMap;

use nohash_hasher::IntSet;
use re_data_store::{LogDb, ObjPath, Timeline};

#[derive(
    Debug, Default, PartialOrd, Ord, enumset::EnumSetType, serde::Deserialize, serde::Serialize,
)]
pub enum ViewCategory {
    #[default]
    Spatial,
    Tensor,
    Text,
    Plot,
}

pub type ViewCategorySet = enumset::EnumSet<ViewCategory>;

pub fn categorize_obj_path(
    timeline: &Timeline,
    log_db: &LogDb,
    obj_path: &ObjPath,
) -> ViewCategorySet {
    crate::profile_function!();

    let Some(obj_type) = log_db.obj_db.types.get(obj_path.obj_type_path())  else {
        return ViewCategorySet::default();
    };

    match obj_type {
        re_log_types::ObjectType::ClassDescription => ViewCategorySet::default(), // we don't have a view for this

        re_log_types::ObjectType::TextEntry => ViewCategory::Text.into(),
        re_log_types::ObjectType::Scalar => ViewCategory::Plot.into(),

        re_log_types::ObjectType::Point2D
        | re_log_types::ObjectType::BBox2D
        | re_log_types::ObjectType::LineSegments2D
        | re_log_types::ObjectType::Point3D
        | re_log_types::ObjectType::Box3D
        | re_log_types::ObjectType::Path3D
        | re_log_types::ObjectType::LineSegments3D
        | re_log_types::ObjectType::Mesh3D => ViewCategory::Spatial.into(),

        re_log_types::ObjectType::Image => {
            // Is it an image or a tensor? Check dimensionality:
            if let Some(timeline_store) = log_db.obj_db.store.get(timeline) {
                if let Some(obj_store) = timeline_store.get(obj_path) {
                    if let Some(field_store) =
                        obj_store.get(&re_data_store::FieldName::new("tensor"))
                    {
                        let time_query = re_data_store::TimeQuery::LatestAt(i64::MAX);
                        if let Ok((_, re_log_types::DataVec::Tensor(tensors))) =
                            field_store.query_field_to_datavec(&time_query, None)
                        {
                            return if tensors
                                .iter()
                                .all(|tensor| tensor.is_shaped_like_an_image())
                            {
                                ViewCategory::Spatial.into()
                            } else {
                                ViewCategory::Tensor.into()
                            };
                        }
                    }
                }
            }

            // something in the query failed - use a sane fallback:
            ViewCategory::Spatial.into()
        }

        re_log_types::ObjectType::Arrow3D => {
            ViewCategory::Spatial.into() // TODO(emilk): implement some sort of entity categorization based on components
        }
    }
}

pub fn group_by_category<'a>(
    timeline: &Timeline,
    log_db: &LogDb,
    objects: impl Iterator<Item = &'a ObjPath>,
) -> BTreeMap<ViewCategory, IntSet<ObjPath>> {
    let mut groups: BTreeMap<ViewCategory, IntSet<ObjPath>> = Default::default();
    for obj_path in objects {
        for category in categorize_obj_path(timeline, log_db, obj_path) {
            groups.entry(category).or_default().insert(obj_path.clone());
        }
    }
    groups
}
