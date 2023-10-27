#include "../error.hpp"
#include "tensor.hpp"

#include <algorithm> // std::min
#include <string>    // std::to_string
#include <utility>   // std::move
#include <vector>

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        /// New Tensor from dimensions and tensor buffer.
        Tensor(
            std::vector<rerun::datatypes::TensorDimension> shape,
            rerun::datatypes::TensorBuffer buffer
        )
            : Tensor(rerun::datatypes::TensorData(std::move(shape), std::move(buffer))) {}

        /// Update the `names` of the contained [`TensorData`] dimensions.
        ///
        /// Any existing Dimension names will be be overwritten.
        ///
        /// If too many, or too few names are provided, this function will call
        /// Error::handle and then proceed to only update the subset of names that it can.
        ///
        /// TODO(#3794): don't use std::vector here.
        Tensor with_dim_names(std::vector<std::string> names) &&;

        // [CODEGEN COPY TO HEADER END]
#endif

        Tensor Tensor::with_dim_names(std::vector<std::string> names) && {
            auto& shape = data.data.shape;

            if (names.size() != shape.size()) {
                Error(
                    ErrorCode::InvalidTensorDimension,
                    "Wrong number of names provided for tensor dimension. " +
                        std::to_string(names.size()) + " provided but " +
                        std::to_string(shape.size()) + " expected."
                )
                    .handle();
            }

            for (size_t i = 0; i < std::min(shape.size(), names.size()); ++i) {
                shape[i].name = std::move(names[i]);
            }

            return std::move(*this);
        }

    } // namespace archetypes
} // namespace rerun
