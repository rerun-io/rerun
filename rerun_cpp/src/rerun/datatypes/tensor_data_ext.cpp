#include "tensor_data.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct TensorDataExt {
#define TensorData TensorDataExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct a 1D tensor with the given buffer.
            static TensorData one_dim(rerun::datatypes::TensorBuffer buffer) {
                auto data = TensorData{};
                data.shape.emplace_back(rerun::datatypes::TensorDimension(buffer.num_elems()));
                data.buffer = std::move(buffer);
                return data;
            }

            // [CODEGEN COPY TO HEADER END]
        };

#undef TensorData
#else
#define TensorDataExt TensorData
#endif

    } // namespace datatypes
} // namespace rerun
