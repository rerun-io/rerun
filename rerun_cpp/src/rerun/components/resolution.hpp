// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/resolution.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/vec2d.hpp"
#include "../result.hpp"

#include <array>
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
        /// **Component**: Pixel resolution width & height, e.g. of a camera sensor.
        ///
        /// Typically in integer units, but for some use cases floating point may be used.
        struct Resolution {
            rerun::datatypes::Vec2D resolution;

            /// Name of the component, used for serialization.
            static const char NAME[];

          public:
            Resolution() = default;

            Resolution(rerun::datatypes::Vec2D resolution_) : resolution(std::move(resolution_)) {}

            Resolution& operator=(rerun::datatypes::Vec2D resolution_) {
                resolution = std::move(resolution_);
                return *this;
            }

            Resolution(std::array<float, 2> xy_) : resolution(std::move(xy_)) {}

            Resolution& operator=(std::array<float, 2> xy_) {
                resolution = std::move(xy_);
                return *this;
            }

            Resolution(const float (&xy_)[2]) : resolution(std::array{xy_[0], xy_[1]}) {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::FixedSizeListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::FixedSizeListBuilder* builder, const Resolution* elements,
                size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of Resolution components.
            static Result<rerun::DataCell> to_data_cell(
                const Resolution* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
