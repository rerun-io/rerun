// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/line_strip3d.fbs".

#pragma once

#include "../collection.hpp"
#include "../data_cell.hpp"
#include "../datatypes/vec3d.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <utility>

namespace arrow {
    class DataType;
    class ListBuilder;
} // namespace arrow

namespace rerun::components {
    /// **Component**: A line strip in 3D space.
    ///
    /// A line strip is a list of points connected by line segments. It can be used to draw
    /// approximations of smooth curves.
    ///
    /// The points will be connected in order, like so:
    /// ```text
    ///        2------3     5
    ///       /        \   /
    /// 0----1          \ /
    ///                  4
    /// ```
    struct LineStrip3D {
        rerun::Collection<rerun::datatypes::Vec3D> points;

      public:
        LineStrip3D() = default;

        LineStrip3D(rerun::Collection<rerun::datatypes::Vec3D> points_)
            : points(std::move(points_)) {}

        LineStrip3D& operator=(rerun::Collection<rerun::datatypes::Vec3D> points_) {
            points = std::move(points_);
            return *this;
        }
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::LineStrip3D> {
        static constexpr const char Name[] = "rerun.components.LineStrip3D";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::ListBuilder* builder, const components::LineStrip3D* elements,
            size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of `rerun::components::LineStrip3D` components.
        static Result<rerun::DataCell> to_arrow(
            const components::LineStrip3D* instances, size_t num_instances
        );
    };
} // namespace rerun
