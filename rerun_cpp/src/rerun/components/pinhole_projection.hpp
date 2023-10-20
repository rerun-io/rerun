// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/pinhole_projection.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/mat3x3.hpp"
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
        /// **Component**: Camera projection, from image coordinates to view coordinates.
        ///
        /// Child from parent.
        /// Image coordinates from camera view coordinates.
        ///
        /// Example:
        /// ```text
        /// 1496.1     0.0  980.5
        ///    0.0  1496.1  744.5
        ///    0.0     0.0    1.0
        /// ```
        struct PinholeProjection {
            rerun::datatypes::Mat3x3 image_from_camera;

            /// Name of the component, used for serialization.
            static const char NAME[];

          public:
            PinholeProjection() = default;

            PinholeProjection(rerun::datatypes::Mat3x3 image_from_camera_)
                : image_from_camera(image_from_camera_) {}

            PinholeProjection& operator=(rerun::datatypes::Mat3x3 image_from_camera_) {
                image_from_camera = image_from_camera_;
                return *this;
            }

            PinholeProjection(std::array<float, 9> flat_columns_)
                : image_from_camera(flat_columns_) {}

            PinholeProjection& operator=(std::array<float, 9> flat_columns_) {
                image_from_camera = flat_columns_;
                return *this;
            }

            PinholeProjection(const float (&flat_columns_)[9])
                : image_from_camera(std::array{
                      flat_columns_[0],
                      flat_columns_[1],
                      flat_columns_[2],
                      flat_columns_[3],
                      flat_columns_[4],
                      flat_columns_[5],
                      flat_columns_[6],
                      flat_columns_[7],
                      flat_columns_[8]}) {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::FixedSizeListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::FixedSizeListBuilder* builder, const PinholeProjection* elements,
                size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of PinholeProjection components.
            static Result<rerun::DataCell> to_data_cell(
                const PinholeProjection* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
