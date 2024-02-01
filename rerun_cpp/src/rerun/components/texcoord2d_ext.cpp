#include "texcoord2d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Construct Texcoord2D from u/v values.
        Texcoord2D(float u, float v) : uv{u, v} {}

        float u() const {
            return uv.x();
        }

        float v() const {
            return uv.y();
        }

        // </CODEGEN_COPY_TO_HEADER>
    };
#endif
} // namespace components
} // namespace rerun
