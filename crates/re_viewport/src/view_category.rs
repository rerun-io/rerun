use re_arrow_store::{LatestAtQuery, TimeInt};
use re_components::{
    Arrow3D, Box3D, Component as _, LineStrip2D, LineStrip3D, Mesh3D, Pinhole, Point2D, Point3D,
    Rect2D, Scalar, Tensor, TextBox, TextEntry, Transform3D,
};
use re_data_store::{EntityPath, StoreDb, Timeline};

#[derive(
    Debug, Default, PartialOrd, Ord, enumset::EnumSetType, serde::Deserialize, serde::Serialize,
)]
pub enum ViewCategory {
    // Ordered by dimensionality
    //
    /// Text log view (text over time)
    Text,

    /// Single textbox element
    TextBox,

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
    pub fn icon(self) -> &'static re_ui::Icon {
        match self {
            ViewCategory::Text => &re_ui::icons::SPACE_VIEW_TEXT,
            ViewCategory::TextBox => &re_ui::icons::SPACE_VIEW_TEXTBOX,
            ViewCategory::TimeSeries => &re_ui::icons::SPACE_VIEW_SCATTERPLOT,
            ViewCategory::BarChart => &re_ui::icons::SPACE_VIEW_HISTOGRAM,
            ViewCategory::Spatial => &re_ui::icons::SPACE_VIEW_3D,
            ViewCategory::Tensor => &re_ui::icons::SPACE_VIEW_TENSOR,
        }
    }
}

impl std::fmt::Display for ViewCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ViewCategory::Text => "Text",
            ViewCategory::TextBox => "Text Box",
            ViewCategory::TimeSeries => "Time Series",
            ViewCategory::BarChart => "Bar Chart",
            ViewCategory::Spatial => "Spatial",
            ViewCategory::Tensor => "Tensor",
        })
    }
}

pub type ViewCategorySet = enumset::EnumSet<ViewCategory>;

// TODO(cmc): these `categorize_*` functions below are pretty dangerous: make sure you've covered
// all possible `ViewCategory` values, or you're in for a bad time..!

pub fn categorize_entity_path(
    timeline: Timeline,
    store_db: &StoreDb,
    entity_path: &EntityPath,
) -> ViewCategorySet {
    re_tracing::profile_function!();

    let mut set = ViewCategorySet::default();

    for component in store_db
        .entity_db
        .data_store
        .all_components(&timeline, entity_path)
        .unwrap_or_default()
    {
        if component == TextEntry::name() {
            set.insert(ViewCategory::Text);
        } else if component == TextBox::name() {
            set.insert(ViewCategory::TextBox);
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
            || component == Transform3D::name()
            || component == Pinhole::name()
        {
            set.insert(ViewCategory::Spatial);
        } else if component == Tensor::name() {
            let timeline_query = LatestAtQuery::new(timeline, TimeInt::MAX);

            let store = &store_db.entity_db.data_store;
            if let Some(tensor) =
                store.query_latest_component::<Tensor>(entity_path, &timeline_query)
            {
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

    set
}
