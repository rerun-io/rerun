#include <utility>
#include "utf8pair.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace encodings {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates a string pair.
        Utf8Pair(rerun::encodings::Utf8 first_, rerun::encodings::Utf8 second_)
            : first(std::move(first_)), second(std::move(second_)) {}

        // </CODEGEN_COPY_TO_HEADER>
#endif
    } // namespace encodings
} // namespace rerun
