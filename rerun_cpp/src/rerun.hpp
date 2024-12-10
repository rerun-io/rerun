#pragma once

// Built-in Rerun types (largely generated from an interface definition language)
#include "rerun/archetypes.hpp"
#include "rerun/components.hpp"
#include "rerun/datatypes.hpp"

// Rerun API.
#include "rerun/collection.hpp"
#include "rerun/collection_adapter.hpp"
#include "rerun/collection_adapter_builtins.hpp"
#include "rerun/component_descriptor.hpp"
#include "rerun/config.hpp"
#include "rerun/entity_path.hpp"
#include "rerun/error.hpp"
#include "rerun/image_utils.hpp"
#include "rerun/recording_stream.hpp"
#include "rerun/result.hpp"
#include "rerun/sdk_info.hpp"
#include "rerun/spawn.hpp"

/// All Rerun C++ types and functions are in the `rerun` namespace or one of its nested namespaces.
namespace rerun {
    /// When an external [`DataLoader`] is asked to load some data that it doesn't know how to load, it
    /// should exit with this exit code.
    // NOTE: Always keep in sync with other languages.
    const int EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE = 66;

    // Archetypes are the quick-and-easy default way of logging data to Rerun.
    // Make them available in the rerun namespace.
    using namespace archetypes;

    // Also import any component or datatype that has a unique name:
    using components::AlbedoFactor;
    using components::Color;
    using components::FillMode;
    using components::GeoLineString;
    using components::HalfSize2D;
    using components::HalfSize3D;
    using components::LatLon;
    using components::LineStrip2D;
    using components::LineStrip3D;
    using components::MediaType;
    using components::Position2D;
    using components::Position3D;
    using components::Radius;
    using components::Text;
    using components::TextLogLevel;
    using components::TransformRelation;
    using components::TriangleIndices;
    using components::Vector2D;
    using components::Vector3D;

    using datatypes::Angle;
    using datatypes::AnnotationInfo;
    using datatypes::ChannelDatatype;
    using datatypes::ClassDescription;
    using datatypes::ColorModel;
    using datatypes::DVec2D;
    using datatypes::Float32;
    using datatypes::KeypointPair;
    using datatypes::Mat3x3;
    using datatypes::PixelFormat;
    using datatypes::Quaternion;
    using datatypes::Rgba32;
    using datatypes::RotationAxisAngle;
    using datatypes::TensorBuffer;
    using datatypes::TensorData;
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
