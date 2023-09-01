// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/archetypes/image.fbs"

#pragma once

#include "../arrow.hpp"
#include "../components/tensor_data.hpp"
#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
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

          public:
            Image() = default;

            Image(rerun::components::TensorData _data) : data(std::move(_data)) {}

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }

            /// Creates a list of Rerun DataCell from this archetype.
            Result<std::vector<rerun::DataCell>> to_data_cells() const;
        };
    } // namespace archetypes
} // namespace rerun
