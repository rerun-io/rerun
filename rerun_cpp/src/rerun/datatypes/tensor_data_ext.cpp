#include "tensor_data.hpp"

// Uncomment for better auto-complete while editing the extension.
//#define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
#define TensorData TensorDataExt

        // [CODEGEN COPY TO HEADER START]

        /// Construct a 1D tensor with the given buffer.
        static TensorData one_dim(rerun::datatypes::TensorBuffer buffer) {
            auto data = TensorData{};
            data.shape.emplace_back(rerun::datatypes::TensorDimension(buffer.num_elems()));
            data.buffer = std::move(buffer);
            return data;
        }

        // TODO(#3794): There should be the option to not have TensorData take ownership of the buffer.
        TensorData(
            std::vector<rerun::datatypes::TensorDimension> shape_,
            rerun::datatypes::TensorBuffer buffer_
        )
            : shape(std::move(shape_)), buffer(std::move(buffer_)) {}

        // [CODEGEN COPY TO HEADER END]
    };
#endif

} // namespace datatypes
} // namespace rerun
