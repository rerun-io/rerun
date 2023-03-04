//! The Rerun SDK
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#![warn(missing_docs)] // Let's keep the this crate well-documented!

// Send data to a rerun session
#[cfg(not(target_arch = "wasm32"))]
mod file_writer;
mod global;
mod log_sink;
mod msg_sender;
mod session;

pub use self::global::{global_session, global_session_with_default_enabled};
pub use self::msg_sender::{MsgSender, MsgSenderError};
pub use self::session::Session;
pub use log_sink::LogSink;

#[cfg(feature = "demo")]
pub mod demo_util;

pub use re_sdk_comms::default_server_addr;

/// Things directly related to logging.
pub mod log {
    pub use re_log_types::{
        msg_bundle::{ComponentBundle, MsgBundle},
        LogMsg, MsgId, PathOp,
    };
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

    #[cfg(feature = "image")]
    pub use re_log_types::external::image;
}

// ---

pub use re_log_types::{
    msg_bundle::{Component, SerializableComponent},
    ApplicationId, ComponentName, EntityPath, RecordingId,
};

const RERUN_ENV_VAR: &str = "RERUN";

/// Helper to get the value of the `RERUN` environment variable.
fn get_rerun_env() -> Option<bool> {
    std::env::var(RERUN_ENV_VAR)
        .ok()
        .and_then(|s| match s.to_lowercase().as_str() {
            "0" | "false" | "off" => Some(false),
            "1" | "true" | "on" => Some(true),
            _ => {
                re_log::warn!(
                    "Invalid value for environment variable {RERUN_ENV_VAR}={s:?}. Expected 'on' or 'off'. It will be ignored"
                );
                None
            }
        })
}

/// Checks the `RERUN` environment variable. If not found, returns the argument.
///
/// Also adds some helpful logging.
fn decide_logging_enabled(default_enabled: bool) -> bool {
    match get_rerun_env() {
        Some(true) => {
            re_log::info!(
                "Rerun Logging is enabled by the '{RERUN_ENV_VAR}' environment variable."
            );
            true
        }
        Some(false) => {
            re_log::info!(
                "Rerun Logging is disabled by the '{RERUN_ENV_VAR}' environment variable."
            );
            false
        }
        None => {
            if !default_enabled {
                re_log::info!(
                    "Rerun Logging has been disabled. Turn it on with the '{RERUN_ENV_VAR}' environment variable."
                );
            }
            default_enabled
        }
    }
}
