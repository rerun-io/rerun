//! The Rerun SDK
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// Send data to a rerun session
mod global;
mod msg_sender;
mod session;

pub use self::global::global_session;
pub use self::msg_sender::{MsgSender, MsgSenderError};
pub use self::session::Session;

// ---

pub use re_log_types::{
    msg_bundle::{Component, SerializableComponent},
    ApplicationId, ComponentName, EntityPath, RecordingId,
};

pub mod log {
    pub use re_log_types::{
        msg_bundle::{ComponentBundle, MsgBundle},
        LogMsg, MsgId, PathOp,
    };
    pub use re_sdk_comms::default_server_addr;
}

pub mod time {
    pub use re_log_types::{Time, TimeInt, TimePoint, TimeType, Timeline};
}

pub mod components {
    pub use re_log_types::component_types::{
        coordinates::{Axis3, Handedness, Sign, SignedAxis3},
        AnnotationContext, AnnotationInfo, Arrow3D, Box3D, ClassDescription, ClassId, ColorRGBA,
        EncodedMesh3D, InstanceKey, KeypointId, Label, LineStrip2D, LineStrip3D, Mat3x3, Mesh3D,
        MeshFormat, MeshId, Pinhole, Point2D, Point3D, Quaternion, Radius, RawMesh3D, Rect2D,
        Rigid3, Scalar, ScalarPlotProps, Size3D, Tensor, TensorData, TensorDataMeaning,
        TensorDimension, TensorId, TensorTrait, TextEntry, Transform, Vec2D, Vec3D, Vec4D,
        ViewCoordinates,
    };
}

// re-exports
pub mod external {
    pub use re_log;
    pub use re_log_types;
    pub use re_memory;
    pub use re_sdk_comms;

    #[cfg(feature = "glam")]
    pub use re_log_types::external::glam;
}
