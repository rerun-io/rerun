#include <utility>
#include "tensor_dimension.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Nameless dimension.
        TensorDimension(size_t size_) : size(size_) {}

        /// Dimension with name.
        TensorDimension(size_t size_, std::string name_) : size(size_), name(std::move(name_)) {}

        // </CODEGEN_COPY_TO_HEADER>
#endif
    } // namespace archetypes
} // namespace rerun
