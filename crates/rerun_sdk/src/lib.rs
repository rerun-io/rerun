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

// TODO
pub use re_log_types::msg_bundle::MsgBundle;
// TODO: PR to rename obj_path
pub use re_log_types::{LogMsg, MsgId, ObjPath as EntityPath};

// Components and datatypes.
//
// TODO:
// - explain
// - separate components vs. primitive types
pub use re_log_types::field_types::AnnotationContext;
pub use re_log_types::field_types::Arrow3D;
pub use re_log_types::field_types::Box3D;
pub use re_log_types::field_types::ClassId;
pub use re_log_types::field_types::ColorRGBA;
pub use re_log_types::field_types::Instance;
pub use re_log_types::field_types::KeypointId;
pub use re_log_types::field_types::Label;
pub use re_log_types::field_types::Mat3x3;
pub use re_log_types::field_types::Quaternion;
pub use re_log_types::field_types::Radius;
pub use re_log_types::field_types::Rect2D;
pub use re_log_types::field_types::Size3D;
pub use re_log_types::field_types::TextEntry;
pub use re_log_types::field_types::UVec2D;
pub use re_log_types::field_types::ViewCoordinates;
pub use re_log_types::field_types::{EncodedMesh3D, Mesh3D, MeshFormat, MeshId, RawMesh3D};
pub use re_log_types::field_types::{LineStrip2D, LineStrip3D};
pub use re_log_types::field_types::{Pinhole, Rigid3, Transform};
pub use re_log_types::field_types::{Point2D, Point3D};
pub use re_log_types::field_types::{Scalar, ScalarPlotProps};
pub use re_log_types::field_types::{
    Tensor, TensorData, TensorDataMeaning, TensorDimension, TensorId, TensorTrait,
};
pub use re_log_types::field_types::{Vec2D, Vec3D, Vec4D};

pub mod reexports {
    pub use re_log;
    pub use re_log_types;
    pub use re_memory;
}
