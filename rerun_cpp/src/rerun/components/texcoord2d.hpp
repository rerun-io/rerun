// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/texcoord2d.fbs".

#pragma once

#include "../datatypes/vec2d.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: A 2D texture UV coordinate.
    ///
    /// Texture coordinates specify a position on a 2D texture.
    /// A range from 0-1 covers the entire texture in the respective dimension.
    /// Unless configured otherwise, the texture repeats outside of this range.
    /// Rerun uses top-left as the origin for UV coordinates.
    ///
    ///   0     U     1
    /// 0 + --------- →
    ///   |           .
    /// V |           .
    ///   |           .
    /// 1 ↓ . . . . . .
    ///
    /// This is the same convention as in Vulkan/Metal/DX12/WebGPU, but (!) unlike OpenGL,
    /// which places the origin at the bottom-left.
    struct Texcoord2D {
        rerun::datatypes::Vec2D uv;

      public:
        // Extensions to generated type defined in 'texcoord2d_ext.cpp'

        /// Construct Texcoord2D from u/v values.
        Texcoord2D(float u, float v) : uv{u, v} {}

        float u() const {
            return uv.x();
        }

        float v() const {
            return uv.y();
        }

      public:
        Texcoord2D() = default;

        Texcoord2D(rerun::datatypes::Vec2D uv_) : uv(uv_) {}

        Texcoord2D& operator=(rerun::datatypes::Vec2D uv_) {
            uv = uv_;
            return *this;
        }

        Texcoord2D(std::array<float, 2> xy_) : uv(xy_) {}

        Texcoord2D& operator=(std::array<float, 2> xy_) {
            uv = xy_;
            return *this;
        }

        /// Cast to the underlying Vec2D datatype
        operator rerun::datatypes::Vec2D() const {
            return uv;
        }
    };
} // namespace rerun::components

namespace rerun {
    /// \private
    template <>
    struct Loggable<components::Texcoord2D> {
        using TypeFwd = rerun::datatypes::Vec2D;
        static_assert(sizeof(TypeFwd) == sizeof(components::Texcoord2D));
        static constexpr const char Name[] = "rerun.components.Texcoord2D";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<TypeFwd>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::Texcoord2D` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::Texcoord2D* instances, size_t num_instances
        ) {
            return Loggable<TypeFwd>::to_arrow(
                reinterpret_cast<const TypeFwd*>(instances),
                num_instances
            );
        }
    };
} // namespace rerun
