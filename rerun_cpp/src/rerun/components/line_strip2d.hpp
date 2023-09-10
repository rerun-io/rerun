// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/line_strip2d.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/vec2d.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <utility>
#include <vector>

namespace arrow {
    class DataType;
    class ListBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace components {
        /// A line strip in 2D space.
        ///
        /// A line strip is a list of points connected by line segments. It can be used to draw
        /// approximations of smooth curves.
        ///
        /// The points will be connected in order, like so:
        ///```text
        ///        2------3     5
        ///       /        \   /
        /// 0----1          \ /
        ///                  4
        ///```
        struct LineStrip2D {
            std::vector<rerun::datatypes::Vec2D> points;

            /// Name of the component, used for serialization.
            static const char* NAME;

          public:
            LineStrip2D() = default;

            LineStrip2D(std::vector<rerun::datatypes::Vec2D> _points)
                : points(std::move(_points)) {}

            LineStrip2D& operator=(std::vector<rerun::datatypes::Vec2D> _points) {
                points = std::move(_points);
                return *this;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::ListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::ListBuilder* builder, const LineStrip2D* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of LineStrip2D components.
            static Result<rerun::DataCell> to_data_cell(
                const LineStrip2D* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
