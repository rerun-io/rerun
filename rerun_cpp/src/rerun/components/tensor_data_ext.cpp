#include "tensor_data.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct TensorDataExt {
#define TensorData TensorDataExt

            // [CODEGEN COPY TO HEADER START]

            /// New Tensor from dimensions and tensor buffer.
            TensorData(
                std::vector<rerun::datatypes::TensorDimension> shape,
                rerun::datatypes::TensorBuffer buffer
            )
                : data(rerun::datatypes::TensorData(std::move(shape), std::move(buffer))) {}

            // [CODEGEN COPY TO HEADER END]
        };

#undef TensorData
#else
#define TensorDataExt TensorData
#endif

    } // namespace components
} // namespace rerun
