// NOTE: Have a look at `re_sdk/src/lib.rs` for an accurate listing of all these symbols.
pub use re_sdk::*;

/// Transform helpers, for use with [`archetypes::Transform3D`].
pub mod transform {
    pub use re_types::datatypes::{Angle, Quaternion, RotationAxisAngle};
}

/// Coordinate system helpers, for use with [`components::ViewCoordinates`].
pub mod coordinates {
    pub use re_types::view_coordinates::{Axis3, Handedness, Sign, SignedAxis3};
}

pub use re_types::{archetypes, components, datatypes};

mod prelude {
    // Import all archetypes into the global namespace to minimize
    // the amount of typing for our users.
    pub use re_types::archetypes::*;

    // Special utility types.
    pub use re_types::Rotation3D;

    // Also import any component or datatype that has a unique name:
    pub use re_chunk::ChunkTimeline;
    pub use re_types::components::{
        AlbedoFactor, ChannelDatatype, Color, ColorModel, FillMode, HalfSize2D, HalfSize3D,
        LineStrip2D, LineStrip3D, MediaType, PixelFormat, Position2D, Position3D, Radius,
        Resolution2D, Scale3D, Text, TextLogLevel, TransformRelation, TriangleIndices, Vector2D,
        Vector3D,
    };
    pub use re_types::datatypes::{
        Angle, AnnotationInfo, ClassDescription, Float32, KeypointPair, Mat3x3, Quaternion, Rgba32,
        RotationAxisAngle, TensorBuffer, TensorData, TensorDimension, Vec2D, Vec3D, Vec4D,
    };
}
pub use prelude::*;
