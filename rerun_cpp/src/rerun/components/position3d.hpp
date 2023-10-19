// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/position3d.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/vec3d.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
    class FixedSizeListBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace components {
        /// **Component**: A position in 3D space.
        struct Position3D {
            rerun::datatypes::Vec3D xyz;

            /// Name of the component, used for serialization.
            static const char NAME[];

          public:
            // Extensions to generated type defined in 'position3d_ext.cpp'

            /// Construct Position3D from x/y/z values.
            Position3D(float x, float y, float z) : xyz{x, y, z} {}

            float x() const {
                return xyz.x();
            }

            float y() const {
                return xyz.y();
            }

            float z() const {
                return xyz.z();
            }

          public:
            Position3D() = default;

            Position3D(const rerun::datatypes::Vec3D& xyz_) : xyz(xyz_) {}

            Position3D& operator=(const rerun::datatypes::Vec3D& xyz_) {
                xyz = xyz_;
                return *this;
            }

            Position3D(const std::array<float, 3>& arg) : xyz(arg) {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::FixedSizeListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::FixedSizeListBuilder* builder, const Position3D* elements,
                size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of Position3D components.
            static Result<rerun::DataCell> to_data_cell(
                const Position3D* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
