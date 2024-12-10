#include "tensor_data.hpp"

namespace rerun::components {

#if 0
    struct TensorDataExt {
        // <CODEGEN_COPY_TO_HEADER>

        /// New tensor data from shape and tensor buffer.
        ///
        /// \param shape Shape of the tensor.
        /// \param buffer The tensor buffer containing the tensor's data.
        TensorData(
            rerun::Collection<uint64_t> shape,
            rerun::datatypes::TensorBuffer buffer
        )
            : data(rerun::datatypes::TensorData(std::move(shape), std::move(buffer))) {}

        /// New tensor data from dimensions and pointer to tensor data.
        ///
        /// Type must be one of the types supported by `rerun::datatypes::TensorData`.
        /// \param shape Shape of the tensor. Determines the number of elements expected to be in `data_`.
        /// \param data_ Target of the pointer must outlive the archetype.
        template <typename TElement>
        explicit TensorData(Collection<uint64_t> shape, const TElement* data_)
            : data(rerun::datatypes::TensorData(std::move(shape), data_)) {}

        // </CODEGEN_COPY_TO_HEADER>
    };
#endif
} // namespace rerun::components
