#include "pinhole_projection.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct PinholeProjectionExt {
#define PinholeProjection PinholeProjectionExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct a new 3x3 pinhole matrix from a pointer to 9 floats (in column major order).
            static PinholeProjection from_mat3x3(const float* elements) {
                return PinholeProjection(rerun::datatypes::Mat3x3(elements));
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
