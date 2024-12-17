#include "resolution.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../rerun_sdk_export.hpp"

// </CODEGEN_COPY_TO_HEADER>

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct ResolutionExt {
            rerun::datatypes::Vec2D resolution;
#define Resolution ResolutionExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct resolution from width and height floats.
            Resolution(float width, float height) : resolution{width, height} {}

            /// Construct resolution from width and height integers.
            Resolution(int width, int height)
                : resolution{static_cast<float>(width), static_cast<float>(height)} {}

            // </CODEGEN_COPY_TO_HEADER>
        };

#undef ResolutionExt
#endif
    } // namespace components
} // namespace rerun
