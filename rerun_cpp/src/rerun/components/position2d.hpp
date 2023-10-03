// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/position2d.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/vec2d.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <utility>

namespace arrow {
    class DataType;
    class FixedSizeListBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace components {
        /// **Component**: A position in 2D space.
        struct Position2D {
            rerun::datatypes::Vec2D xy;

            /// Name of the component, used for serialization.
            static const char NAME[];

          public:
            // Extensions to generated type defined in 'position2d_ext.cpp'

            /// Construct Position2D from x/y values.
            Position2D(float x, float y) : xy{x, y} {}

            float x() const {
                return xy.x();
            }

            float y() const {
                return xy.y();
            }

          public:
            Position2D() = default;

            Position2D(rerun::datatypes::Vec2D _xy) : xy(std::move(_xy)) {}

            Position2D& operator=(rerun::datatypes::Vec2D _xy) {
                xy = std::move(_xy);
                return *this;
            }

            Position2D(const float (&arg)[2]) : xy(arg) {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::FixedSizeListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::FixedSizeListBuilder* builder, const Position2D* elements,
                size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of Position2D components.
            static Result<rerun::DataCell> to_data_cell(
                const Position2D* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
