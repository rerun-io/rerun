//! The Rerun SDK
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#![warn(missing_docs)] // Let's keep the this crate well-documented!

// Send data to a rerun session
mod global;
mod msg_sender;
mod session;

pub use self::global::global_session;
pub use self::msg_sender::{MsgSender, MsgSenderError};
pub use self::session::Session;

#[cfg(feature = "demo")]
pub mod demo_util;

// ---

pub use re_log_types::{
    msg_bundle::{Component, SerializableComponent},
    ApplicationId, ComponentName, EntityPath, RecordingId,
};

/// Things directly related to logging.
pub mod log {
    pub use re_log_types::{
        msg_bundle::{ComponentBundle, MsgBundle},
        LogMsg, MsgId, PathOp,
    };
    pub use re_sdk_comms::default_server_addr;
}

/// Time-related types.
pub mod time {
    pub use re_log_types::{Time, TimeInt, TimePoint, TimeType, Timeline};
}

/// These are the different _components_ you can log.
///
/// They all implement the [`Component`] trait,
/// and can be used in [`MsgSender::with_component`].
pub mod components {
    pub use re_log_types::component_types::{
        AnnotationContext, AnnotationInfo, Arrow3D, Box3D, ClassDescription, ClassId, ColorRGBA,
        EncodedMesh3D, InstanceKey, KeypointId, Label, LineStrip2D, LineStrip3D, Mat3x3, Mesh3D,
        MeshFormat, MeshId, Pinhole, Point2D, Point3D, Quaternion, Radius, RawMesh3D, Rect2D,
        Rigid3, Scalar, ScalarPlotProps, Size3D, Tensor, TensorData, TensorDataMeaning,
        TensorDimension, TensorId, TensorTrait, TextEntry, Transform, Vec2D, Vec3D, Vec4D,
        ViewCoordinates,
    };
}

/// Coordinate system helpers, for use with [`components::ViewCoordinates`].
pub mod coordinates {
    pub use re_log_types::coordinates::{Axis3, Handedness, Sign, SignedAxis3};
}

/// Re-exports of other crates.
pub mod external {
    pub use re_log;
    pub use re_log_types;
    pub use re_memory;
    pub use re_sdk_comms;

    #[cfg(feature = "glam")]
    pub use re_log_types::external::glam;
}
