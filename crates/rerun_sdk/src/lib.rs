//! The Rerun SDK
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// Work with timestamps
pub mod time;
pub use time::log_time;

// Send data to a rerun session
mod session;
pub use self::session::Session;

mod global;
pub use self::global::global_session;

pub mod viewer;

// TODO(cmc): clean all that up?

pub use re_log_types::msg_bundle::MsgBundle;
pub use re_log_types::{EntityPath, LogMsg, MsgId};
pub use re_log_types::{Time, TimePoint, TimeType, Timeline};

// TODO(cmc): separate datatypes (e.g. Vec3D) from components (e.g. Size3D).
pub use re_log_types::component_types::AnnotationContext;
pub use re_log_types::component_types::Arrow3D;
pub use re_log_types::component_types::Box3D;
pub use re_log_types::component_types::ClassId;
pub use re_log_types::component_types::ColorRGBA;
pub use re_log_types::component_types::Instance;
pub use re_log_types::component_types::KeypointId;
pub use re_log_types::component_types::Label;
pub use re_log_types::component_types::Mat3x3;
pub use re_log_types::component_types::Quaternion;
pub use re_log_types::component_types::Radius;
pub use re_log_types::component_types::Rect2D;
pub use re_log_types::component_types::Size3D;
pub use re_log_types::component_types::TextEntry;
pub use re_log_types::component_types::{
    coordinates::{Axis3, Handedness, Sign, SignedAxis3},
    ViewCoordinates,
};
pub use re_log_types::component_types::{EncodedMesh3D, Mesh3D, MeshFormat, MeshId, RawMesh3D};
pub use re_log_types::component_types::{LineStrip2D, LineStrip3D};
pub use re_log_types::component_types::{Pinhole, Rigid3, Transform};
pub use re_log_types::component_types::{Point2D, Point3D};
pub use re_log_types::component_types::{Scalar, ScalarPlotProps};
pub use re_log_types::component_types::{
    Tensor, TensorData, TensorDataMeaning, TensorDimension, TensorId, TensorTrait,
};
pub use re_log_types::component_types::{Vec2D, Vec3D, Vec4D};

pub mod reexports {
    pub use re_log;
    pub use re_log_types;
    pub use re_memory;
}
