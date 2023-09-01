// DO NOT EDIT!: This file was autogenerated by re_types_builder in
// crates/re_types_builder/src/codegen/cpp/mod.rs:54 Based on
// "crates/re_types/definitions/rerun/archetypes/image.fbs"

#pragma once

#include "../arrow.hpp"
#include "../components/draw_order.hpp"
#include "../components/tensor_data.hpp"
#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// A monochrome or color image.
        ///
        /// The shape of the `TensorData` must be mappable to:
        ///- A `HxW` tensor, treated as a grayscale image.
        ///- A `HxWx3` tensor, treated as an RGB image.
        ///- A `HxWx4` tensor, treated as an RGBA image.
        ///
        /// The viewer has limited support for ignoring extra empty dimensions.
        struct Image {
            /// The image data. Should always be a rank-2 or rank-3 tensor.
            rerun::components::TensorData data;

            /// An optional floating point value that specifies the 2D drawing order.
            /// Objects with higher values are drawn on top of those with lower values.
            ///
            /// The default for 2D points is -10.0.
            std::optional<rerun::components::DrawOrder> draw_order;

          public:
            Image() = default;

            Image(rerun::components::TensorData _data) : data(std::move(_data)) {}

            /// An optional floating point value that specifies the 2D drawing order.
            /// Objects with higher values are drawn on top of those with lower values.
            ///
            /// The default for 2D points is -10.0.
            Image& with_draw_order(rerun::components::DrawOrder _draw_order) {
                draw_order = std::move(_draw_order);
                return *this;
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }

            /// Creates a list of Rerun DataCell from this archetype.
            Result<std::vector<rerun::DataCell>> to_data_cells() const;
        };
    } // namespace archetypes
} // namespace rerun
