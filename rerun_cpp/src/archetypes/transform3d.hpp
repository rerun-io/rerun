// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/archetypes/transform3d.fbs"

#pragma once

#include <cstdint>
#include <utility>

#include "../components/transform3d.hpp"

namespace rr {
    namespace archetypes {
        /// A 3D transform.
        struct Transform3D {
            /// The transform
            rr::components::Transform3D transform;

            Transform3D(rr::components::Transform3D transform) : transform(std::move(transform)) {}
        };
    } // namespace archetypes
} // namespace rr
