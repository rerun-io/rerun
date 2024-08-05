// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/vector2d.fbs".

#pragma once

#include "../datatypes/vec2d.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: A vector in 2D space.
    struct Vector2D {
        rerun::datatypes::Vec2D vector;

      public: // START of extensions from vector2d_ext.cpp:
        /// Construct Vector2D from x/y values.
        Vector2D(float x, float y) : vector{x, y} {}

        /// Construct Vec2D from x/y float pointer.
        explicit Vector2D(const float* xy) : vector{xy[0], xy[1]} {}

        float x() const {
            return vector.x();
        }

        float y() const {
            return vector.y();
        }

        // END of extensions from vector2d_ext.cpp, start of generated code:

      public:
        Vector2D() = default;

        Vector2D(rerun::datatypes::Vec2D vector_) : vector(vector_) {}

        Vector2D& operator=(rerun::datatypes::Vec2D vector_) {
            vector = vector_;
            return *this;
        }

        Vector2D(std::array<float, 2> xy_) : vector(xy_) {}

        Vector2D& operator=(std::array<float, 2> xy_) {
            vector = xy_;
            return *this;
        }

        /// Cast to the underlying Vec2D datatype
        operator rerun::datatypes::Vec2D() const {
            return vector;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Vec2D) == sizeof(components::Vector2D));

    /// \private
    template <>
    struct Loggable<components::Vector2D> {
        static constexpr const char Name[] = "rerun.components.Vector2D";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Vec2D>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::Vector2D` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::Vector2D* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::Vec2D>::to_arrow(&instances->vector, num_instances);
        }
    };
} // namespace rerun
