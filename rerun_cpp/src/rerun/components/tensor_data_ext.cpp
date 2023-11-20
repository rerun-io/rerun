#include "tensor_data.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct TensorDataExt {
#define TensorData TensorDataExt

            // <CODEGEN_COPY_TO_HEADER>

            /// New Tensor from dimensions and tensor buffer.
            TensorData(
                rerun::Collection<rerun::datatypes::TensorDimension> shape,
                rerun::datatypes::TensorBuffer buffer
            )
                : data(rerun::datatypes::TensorData(std::move(shape), std::move(buffer))) {}

            // </CODEGEN_COPY_TO_HEADER>
        };

#undef TensorData
#else
#define TensorDataExt TensorData
#endif

    } // namespace components
} // namespace rerun
