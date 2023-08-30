// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/archetypes/image_base.fbs"

#pragma once

#include "../arrow.hpp"
#include "../components/image_variant.hpp"
#include "../components/tensor_data.hpp"
#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// The base archetype for all Image variants.
        ///
        /// This archetype is not intended to be used directly, but rather to be
        /// used via the `Image`, `SegmentationImage`, and `DepthImage` archetype aliases.
        struct ImageBase {
            /// What variant of image this is.
            rerun::components::ImageVariant variant;

            /// The image data. Should always be a rank-2 or rank-3 tensor.
            rerun::components::TensorData data;

          public:
            ImageBase() = default;

            ImageBase(rerun::components::ImageVariant _variant, rerun::components::TensorData _data)
                : variant(std::move(_variant)), data(std::move(_data)) {}

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }

            /// Creates a list of Rerun DataCell from this archetype.
            Result<std::vector<rerun::DataCell>> to_data_cells() const;
        };
    } // namespace archetypes
} // namespace rerun
