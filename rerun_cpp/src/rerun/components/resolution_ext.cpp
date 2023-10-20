#include "resolution.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct ResolutionExt {
            rerun::datatypes::Vec2D resolution;
#define Resolution ResolutionExt

            // [CODEGEN COPY TO HEADER START]

            static const Resolution IDENTITY;

            /// Construct resolution from width and height floats.
            Resolution(float width, float height) : resolution{width, height} {}

            /// Construct resolution from width and height integers.
            Resolution(int width, int height)
                : resolution{static_cast<float>(width), static_cast<float>(width)} {}

            // [CODEGEN COPY TO HEADER END]
        };

#undef ResolutionExt
#endif
    } // namespace components
} // namespace rerun
