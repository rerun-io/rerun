#include <utility>
#include "utf8pair.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates a string pair.
        Utf8Pair(rerun::datatypes::Utf8 first_, rerun::datatypes::Utf8 second_)
            : first(std::move(first_)), second(std::move(second_)) {}

        // </CODEGEN_COPY_TO_HEADER>
#endif
    } // namespace datatypes
} // namespace rerun
