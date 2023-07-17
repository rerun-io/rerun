// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/translation_rotation_scale3d.fbs"

#pragma once

#include <cstdint>
#include <optional>
#include <vector>

namespace rr {
    namespace datatypes {
        struct TranslationRotationScale3D {
            std::optional<rr::datatypes::Vec3D> translation;
            std::optional<rr::datatypes::Rotation3D> rotation;
            std::optional<rr::datatypes::Scale3D> scale;
            bool from_parent;
        };
    } // namespace datatypes
} // namespace rr
