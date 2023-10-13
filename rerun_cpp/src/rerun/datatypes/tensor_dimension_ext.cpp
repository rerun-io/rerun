#include <utility>
#include "tensor_dimension.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        /// Nameless dimension.
        TensorDimension(size_t size_) : size(size_) {}

        /// Dimension with name.
        TensorDimension(size_t size_, std::string name_) : size(size_), name(std::move(name_)) {}

        // [CODEGEN COPY TO HEADER END]
#endif
    } // namespace archetypes
} // namespace rerun
