use std::collections::BTreeMap;

use nohash_hasher::IntSet;
use re_data_store::{LogDb, ObjPath, Timeline};
use re_log_types::DataPath;

#[derive(
    Debug, Default, PartialOrd, Ord, enumset::EnumSetType, serde::Deserialize, serde::Serialize,
)]
pub enum ViewCategory {
    // Ordered by dimensionality
    //
    /// Text log view (text over time)
    Text,

    /// Time series plot (scalar over time)
    TimeSeries,

    /// Bar-chart plots made from 1D tensor data
    BarChart,

    /// 2D or 3D view
    #[default]
    Spatial,

    /// High-dimensional tensor view
    Tensor,
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
        re_log_types::ObjectType::TextEntry => ViewCategory::Text.into(),

        re_log_types::ObjectType::Scalar => ViewCategory::TimeSeries.into(),

        re_log_types::ObjectType::Point2D
        | re_log_types::ObjectType::BBox2D
        | re_log_types::ObjectType::LineSegments2D
        | re_log_types::ObjectType::Point3D
        | re_log_types::ObjectType::Box3D
        | re_log_types::ObjectType::Path3D
        | re_log_types::ObjectType::LineSegments3D
        | re_log_types::ObjectType::Mesh3D
        | re_log_types::ObjectType::Arrow3D => ViewCategory::Spatial.into(),

        re_log_types::ObjectType::Image => {
            // Some sort of tensor - could be an image, a vector, or a general tensor - let's check!
            if let Some(Ok((_, re_log_types::DataVec::Tensor(tensors)))) =
                log_db.obj_db.store.query_data_path(
                    timeline,
                    &re_data_store::TimeQuery::LatestAt(i64::MAX),
                    &DataPath::new(obj_path.clone(), "tensor".into()),
                )
            {
                if tensors.iter().all(|tensor| tensor.is_vector()) {
                    ViewCategory::BarChart.into()
                } else if tensors
                    .iter()
                    .all(|tensor| tensor.is_shaped_like_an_image())
                {
                    ViewCategory::Spatial.into()
                } else {
                    ViewCategory::Tensor.into()
                }
            } else {
                // something in the query failed - use a sane fallback:
                ViewCategory::Spatial.into()
            }
        }

        re_log_types::ObjectType::ArrowObject => {
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
