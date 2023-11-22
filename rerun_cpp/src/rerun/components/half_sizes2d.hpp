// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/half_sizes2d.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/vec2d.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
    class FixedSizeListBuilder;
} // namespace arrow

namespace rerun::components {
    /// **Component**: Half-sizes (extents) of a 2D box along its local axis, starting at its local origin/center.
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
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::HalfSizes2D> {
        static constexpr const char Name[] = "rerun.components.HalfSizes2D";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FixedSizeListBuilder* builder, const components::HalfSizes2D* elements,
            size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of `rerun::components::HalfSizes2D` components.
        static Result<rerun::DataCell> to_arrow(
            const components::HalfSizes2D* instances, size_t num_instances
        );
    };
} // namespace rerun
