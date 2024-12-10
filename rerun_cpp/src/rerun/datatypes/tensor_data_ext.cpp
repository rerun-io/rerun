#include "tensor_data.hpp"

namespace rerun::datatypes {

#if 0
    // <CODEGEN_COPY_TO_HEADER>

    /// New tensor data from shape and tensor buffer.
    ///
    /// \param shape_ Shape of the tensor.
    /// \param buffer_ The tensor buffer containing the tensor's data.
    TensorData(
        Collection<uint64_t> shape_, datatypes::TensorBuffer buffer_
    )
        : shape(std::move(shape_)), buffer(std::move(buffer_)) {}

    /// New tensor data from dimensions and pointer to tensor data.
    ///
    /// Type must be one of the types supported by `rerun::datatypes::TensorData`.
    /// \param shape_ Shape of the tensor. Determines the number of elements expected to be in `data`.
    /// \param data Target of the pointer must outlive the archetype.
    template <typename TElement>
    explicit TensorData(Collection<uint64_t> shape_, const TElement* data) : shape(std::move(shape_)) {
        size_t num_elements = shape.empty() ? 0 : 1;
        for (const auto& dim : shape) {
            num_elements *= dim;
        }
        buffer = rerun::Collection<TElement>::borrow(data, num_elements);
    }

    // </CODEGEN_COPY_TO_HEADER>
#endif
} // namespace rerun::datatypes
