#include "line_strip2d.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        /// Create line strip from a list of positions, each connected to the next.
        template <typename... Args>
        LineStrip2D(rerun::datatypes::Vec2D a, rerun::datatypes::Vec2D b, Args... more)
            : points({a, b, more...}) {}

        // [CODEGEN COPY TO HEADER END]
#endif
    } // namespace components
} // namespace rerun
