#include "../error.hpp"
#include "tensor.hpp"

#include <algorithm> // std::min
#include <string>    // std::to_string
#include <utility>   // std::move

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {

#if 0
    // <CODEGEN_COPY_TO_HEADER>

    /// New Tensor from dimensions and tensor buffer.
    Tensor(Collection<uint64_t> shape, datatypes::TensorBuffer buffer)
        : Tensor(datatypes::TensorData(std::move(shape), std::move(buffer))) {}

    /// New tensor from dimensions and pointer to tensor data.
    ///
    /// Type must be one of the types supported by `rerun::datatypes::TensorData`.
    /// \param shape
    /// Shape of the image. Determines the number of elements expected to be in `data`.
    /// \param data_
    /// Target of the pointer must outlive the archetype.
    template <typename TElement>
    explicit Tensor(Collection<uint64_t> shape, const TElement* data_)
        : Tensor(datatypes::TensorData(std::move(shape), data_)) {}

    /// Update the `names` of the contained `TensorData` dimensions.
    ///
    /// Any existing Dimension names will be overwritten.
    ///
    /// If too many, or too few names are provided, this function will call
    /// Error::handle and then proceed to only update the subset of names that it can.
    Tensor with_dim_names(Collection<std::string> names) &&;

    // </CODEGEN_COPY_TO_HEADER>
#endif

    Tensor Tensor::with_dim_names(Collection<std::string> names) && {
        auto& shape = data.data.shape;

        if (names.size() != shape.size()) {
            Error(
                ErrorCode::InvalidTensorDimension,
                "Wrong number of names provided for tensor dimension. " +
                    std::to_string(names.size()) + " provided but " + std::to_string(shape.size()) +
                    " expected."
            )
                .handle();
        }

        this->data.data.names = std::move(names);

        return std::move(*this);
    }

} // namespace rerun::archetypes
