// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/half_sizes2d.fbs".

#pragma once

#include "../datatypes/vec2d.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: Half-size (radius) of a 2D box.
    ///
    /// Measured in its local coordinate system.
    ///
    /// The box extends both in negative and positive direction along each axis.
    /// Negative sizes indicate that the box is flipped along the respective axis, but this has no effect on how it is displayed.
    struct HalfSizes2D {
        rerun::datatypes::Vec2D xy;

      public:
        // Extensions to generated type defined in 'half_sizes2d_ext.cpp'

        /// Construct HalfSizes2D from x/y values.
        HalfSizes2D(float x, float y) : xy{x, y} {}

        float x() const {
            return xy.x();
        }

        float y() const {
            return xy.y();
        }

      public:
        HalfSizes2D() = default;

        HalfSizes2D(rerun::datatypes::Vec2D xy_) : xy(xy_) {}

        HalfSizes2D& operator=(rerun::datatypes::Vec2D xy_) {
            xy = xy_;
            return *this;
        }

        HalfSizes2D(std::array<float, 2> xy_) : xy(xy_) {}

        HalfSizes2D& operator=(std::array<float, 2> xy_) {
            xy = xy_;
            return *this;
        }

        /// Cast to the underlying Vec2D datatype
        operator rerun::datatypes::Vec2D() const {
            return xy;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Vec2D) == sizeof(components::HalfSizes2D));

    /// \private
    template <>
    struct Loggable<components::HalfSizes2D> {
        static constexpr const char Name[] = "rerun.components.HalfSizes2D";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Vec2D>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::HalfSizes2D` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::HalfSizes2D* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::Vec2D>::to_arrow(&instances->xy, num_instances);
        }
    };
} // namespace rerun
