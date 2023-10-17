// The Rerun C++ SDK.
#pragma once

// Built-in Rerun types (largely generated from an interface definition language)
#include "rerun/archetypes.hpp"
#include "rerun/components.hpp"
#include "rerun/datatypes.hpp"

// Rerun API.
#include "rerun/error.hpp"
#include "rerun/recording_stream.hpp"
#include "rerun/result.hpp"
#include "rerun/sdk_info.hpp"

// Archetypes are the quick-and-easy default way of logging data to Rerun.
// Make them available in the rerun namespace.
namespace rerun {
    using namespace archetypes;

    // Also import some select, often-used, datatypes and components:
    using components::Color;
    using components::HalfSizes2D;
    using components::HalfSizes3D;
    using components::InstanceKey;
    using components::Material;
    using components::MediaType;
    using components::MeshProperties;
    using components::OutOfTreeTransform3D;
    using components::Position3D;
    using components::Radius;
    using components::TextLogLevel;
    using components::Vector3D;

    using datatypes::Angle;
    using datatypes::ClassDescription;
    using datatypes::Float32;
    using datatypes::KeypointPair;
    using datatypes::Mat3x3;
    using datatypes::Quaternion;
    using datatypes::Rgba32;
    using datatypes::Rotation3D;
    using datatypes::RotationAxisAngle;
    using datatypes::Scale3D;
    using datatypes::TranslationAndMat3x3;
    using datatypes::TranslationRotationScale3D;
} // namespace rerun
