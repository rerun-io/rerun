use re_arrow_store::{LatestAtQuery, TimeInt};
use re_data_store::{query_transform, EntityPath, LogDb, Timeline};
use re_log_types::{
    component_types::{
        Box3D, LineStrip2D, LineStrip3D, Point2D, Point3D, Rect2D, Scalar, Tensor, TensorTrait,
        TextEntry,
    },
    msg_bundle::Component,
    Arrow3D, Mesh3D, Transform,
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

// TODO(cmc): these `categorize_*` functions below are pretty dangerous: make sure you've covered
// all possible `ViewCategory` values, or you're in for a bad time..!

pub fn categorize_entity_path(
    timeline: Timeline,
    log_db: &LogDb,
    entity_path: &EntityPath,
) -> ViewCategorySet {
    crate::profile_function!();

    let mut set = categorize_arrow_entity_path(&timeline, log_db, entity_path);

    // If it has a transform we might want to visualize it in space
    // (as of writing we do that only for projections, i.e. cameras, but visualizations for rigid transforms may be added)
    if query_transform(
        &log_db.entity_db,
        entity_path,
        &LatestAtQuery::new(timeline, TimeInt::MAX),
    )
    .is_some()
    {
        set.insert(ViewCategory::Spatial);
    }

    set
}

pub fn categorize_arrow_entity_path(
    timeline: &Timeline,
    log_db: &LogDb,
    entity_path: &EntityPath,
) -> ViewCategorySet {
    crate::profile_function!();

    log_db
        .entity_db
        .arrow_store
        .all_components(timeline, entity_path)
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
                let timeline_query = LatestAtQuery::new(*timeline, TimeInt::MAX);

                if let Ok(entity_view) = query_entity_with_primary::<Tensor>(
                    &log_db.entity_db.arrow_store,
                    &timeline_query,
                    entity_path,
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
