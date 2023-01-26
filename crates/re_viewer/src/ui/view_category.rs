use re_arrow_store::{LatestAtQuery, TimeInt};
use re_data_store::{query_transform, LogDb, ObjPath, Timeline};
use re_log_types::{
    field_types::{
        Box3D, LineStrip2D, LineStrip3D, Point2D, Point3D, Rect2D, Scalar, Tensor, TensorTrait,
        TextEntry,
    },
    msg_bundle::Component,
    Arrow3D, DataPath, Mesh3D, Transform,
};
use re_query::query_entity_with_primary;

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

impl ViewCategory {
    pub fn icon(&self) -> &'static str {
        match self {
            ViewCategory::Text => "📃",
            ViewCategory::TimeSeries => "📈",
            ViewCategory::BarChart => "📊",
            ViewCategory::Spatial => "🖼",
            ViewCategory::Tensor => "🇹",
        }
    }
}

pub type ViewCategorySet = enumset::EnumSet<ViewCategory>;

pub fn categorize_obj_path(
    timeline: &Timeline,
    log_db: &LogDb,
    obj_path: &ObjPath,
) -> ViewCategorySet {
    crate::profile_function!();

    let Some(obj_type) = log_db.obj_db.types.get(obj_path.obj_type_path()) else {
        // If it has a transform we might want to visualize it in space
        // (as of writing we do that only for projections, i.e. cameras, but visualizations for rigid transforms may be added)
        if query_transform(&log_db.obj_db, timeline, obj_path, Some(i64::MAX)).is_some() {
            return ViewCategory::Spatial.into();
        }

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
            categorize_arrow_obj_path(timeline, log_db, obj_path)
        }
    }
}

pub fn categorize_arrow_obj_path(
    timeline: &Timeline,
    log_db: &LogDb,
    obj_path: &ObjPath,
) -> ViewCategorySet {
    crate::profile_function!();

    log_db
        .obj_db
        .arrow_store
        .all_components(timeline, obj_path)
        .unwrap_or_default()
        .into_iter()
        .fold(ViewCategorySet::default(), |mut set, component| {
            if component == TextEntry::name() {
                set.insert(ViewCategory::Text);
            } else if component == Scalar::name() {
                set.insert(ViewCategory::TimeSeries);
            } else if component == Point2D::name()
                || component == Point3D::name()
                || component == Rect2D::name()
                || component == Box3D::name()
                || component == LineStrip2D::name()
                || component == LineStrip3D::name()
                || component == Mesh3D::name()
                || component == Arrow3D::name()
                || component == Transform::name()
            {
                set.insert(ViewCategory::Spatial);
            } else if component == Tensor::name() {
                let timeline_query = LatestAtQuery::new(*timeline, TimeInt::from(i64::MAX));

                if let Ok(entity_view) = query_entity_with_primary::<Tensor>(
                    &log_db.obj_db.arrow_store,
                    &timeline_query,
                    obj_path,
                    &[],
                ) {
                    if let Ok(iter) = entity_view.iter_primary() {
                        for tensor in iter.flatten() {
                            if tensor.is_vector() {
                                set.insert(ViewCategory::BarChart);
                            } else if tensor.is_shaped_like_an_image() {
                                set.insert(ViewCategory::Spatial);
                            } else {
                                set.insert(ViewCategory::Tensor);
                            }
                        }
                    }
                }
            }
            set
        })
}
