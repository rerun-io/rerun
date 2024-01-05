// NOTE: Have a look at `re_sdk/src/lib.rs` for an accurate listing of all these symbols.
pub use re_sdk::*;

/// Transform helpers, for use with [`components::Transform3D`].
pub mod transform {
    pub use re_types::datatypes::{
        Angle, Rotation3D, RotationAxisAngle, Scale3D, Transform3D, TranslationAndMat3x3,
        TranslationRotationScale3D,
    };
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

    // Also import any component or datatype that has a unique name:
    pub use re_types::components::{
        Color, HalfSizes2D, HalfSizes3D, InstanceKey, LineStrip2D, LineStrip3D, Material,
        MediaType, MeshProperties, OutOfTreeTransform3D, Position2D, Position3D, Radius, Text,
        TextLogLevel, Vector2D, Vector3D,
    };
    pub use re_types::datatypes::{
        Angle, AnnotationInfo, ClassDescription, Float32, KeypointPair, Mat3x3, Quaternion, Rgba32,
        Rotation3D, RotationAxisAngle, Scale3D, TensorBuffer, TensorData, TensorDimension,
        TranslationAndMat3x3, TranslationRotationScale3D, Vec2D, Vec3D, Vec4D,
    };
}
pub use prelude::*;
