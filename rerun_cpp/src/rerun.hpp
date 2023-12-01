#pragma once

// Built-in Rerun types (largely generated from an interface definition language)
#include "rerun/archetypes.hpp"
#include "rerun/components.hpp"
#include "rerun/datatypes.hpp"

// Rerun API.
#include "rerun/collection.hpp"
#include "rerun/collection_adapter.hpp"
#include "rerun/collection_adapter_builtins.hpp"
#include "rerun/config.hpp"
#include "rerun/error.hpp"
#include "rerun/recording_stream.hpp"
#include "rerun/result.hpp"
#include "rerun/sdk_info.hpp"
#include "rerun/spawn.hpp"

/// All Rerun C++ types and functions are in the `rerun` namespace or one of its nested namespaces.
namespace rerun {
    // Archetypes are the quick-and-easy default way of logging data to Rerun.
    // Make them available in the rerun namespace.
    using namespace archetypes;

    // Also import any component or datatype that has a unique name:
    using components::Color;
    using components::HalfSizes2D;
    using components::HalfSizes3D;
    using components::InstanceKey;
    using components::LineStrip2D;
    using components::LineStrip3D;
    using components::Material;
    using components::MediaType;
    using components::MeshProperties;
    using components::OutOfTreeTransform3D;
    using components::Position2D;
    using components::Position3D;
    using components::Radius;
    using components::Text;
    using components::TextLogLevel;
    using components::Vector3D;

    using datatypes::Angle;
    using datatypes::AnnotationInfo;
    using datatypes::ClassDescription;
    using datatypes::Float32;
    using datatypes::KeypointPair;
    using datatypes::Mat3x3;
    using datatypes::Quaternion;
    using datatypes::Rgba32;
    using datatypes::Rotation3D;
    using datatypes::RotationAxisAngle;
    using datatypes::Scale3D;
    using datatypes::TensorBuffer;
    using datatypes::TensorData;
    using datatypes::TensorDimension;
    using datatypes::TranslationAndMat3x3;
    using datatypes::TranslationRotationScale3D;
    using datatypes::Vec2D;
    using datatypes::Vec3D;
    using datatypes::Vec4D;

    // Document namespaces that span several files:

    /// All built-in archetypes. See [Types](https://www.rerun.io/docs/reference/types) in the Rerun manual.
    namespace archetypes {}

    /// All built-in components. See [Types](https://www.rerun.io/docs/reference/types) in the Rerun manual.
    namespace components {}

    /// All built-in datatypes. See [Types](https://www.rerun.io/docs/reference/types) in the Rerun manual.
    namespace datatypes {}

    /// All blueprint types. This is still experimental and subject to change!
    namespace blueprint {}
} // namespace rerun
