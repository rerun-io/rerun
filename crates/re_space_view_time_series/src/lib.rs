//! Rerun time series Space View
//!
//! A Space View that shows plots over Rerun timelines.

mod space_view_class;
mod view_part_system;

pub use space_view_class::TimeSeriesSpaceView;

pub(crate) use self::space_view_class::TimeSeriesSpaceViewFeedback;
