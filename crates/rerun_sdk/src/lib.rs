//! The Rerun SDK
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// Work with timestamps
// TODO(cmc): remove traces of previous APIs & examples
pub mod time;
pub use time::log_time;

// Send data to a rerun session
mod session;
pub use self::session::Session;

mod msg_sender;
pub use self::msg_sender::{MsgSender, MsgSenderError};

mod global;
pub use self::global::global_session;

pub mod viewer;

// ---

// init
pub use re_log_types::ApplicationId;
pub use re_sdk_comms::default_server_addr;

// messages
pub use re_log_types::{
    msg_bundle::{Component, ComponentBundle, MsgBundle, SerializableComponent},
    ComponentName, EntityPath, LogMsg, MsgId, Time, TimeInt, TimePoint, TimeType, Timeline,
};

// components
pub use re_log_types::component_types::{
    coordinates::{Axis3, Handedness, Sign, SignedAxis3},
    AnnotationContext, Arrow3D, Box3D, ClassId, ColorRGBA, EncodedMesh3D, InstanceKey, KeypointId,
    Label, LineStrip2D, LineStrip3D, Mat3x3, Mesh3D, MeshFormat, MeshId, Pinhole, Point2D, Point3D,
    Quaternion, Radius, RawMesh3D, Rect2D, Rigid3, Scalar, ScalarPlotProps, Size3D, Tensor,
    TensorData, TensorDataMeaning, TensorDimension, TensorId, TensorTrait, TextEntry, Transform,
    Vec2D, Vec3D, Vec4D, ViewCoordinates,
};

// re-exports
pub mod external {
    pub use re_log;
    pub use re_log_types;
    pub use re_memory;
    pub use re_sdk_comms;
}
