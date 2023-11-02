#include "tensor_data.hpp"

// Uncomment for better auto-complete while editing the extension.
//#define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
#define TensorData TensorDataExt

        // <CODEGEN_COPY_TO_HEADER>

        // TODO(#3794): There should be the option to not have TensorData take ownership of the buffer.
        TensorData(
            std::vector<rerun::datatypes::TensorDimension> shape_,
            rerun::datatypes::TensorBuffer buffer_
        )
            : shape(std::move(shape_)), buffer(std::move(buffer_)) {}

        // </CODEGEN_COPY_TO_HEADER>
#endif
    } // namespace datatypes
} // namespace rerun
