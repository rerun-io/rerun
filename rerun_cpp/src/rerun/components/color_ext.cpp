#include "color.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct ColorExt : public Color {
            ColorExt(uint32_t _rgba) : Color(_rgba) {}

#define Color ColorExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Color from unmultiplied RGBA values.
            Color(uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255)
                : Color((r << 24) | (g << 16) | (b << 8) | a) {}

            /// Construct Color from unmultiplied RGBA values.
            Color(uint8_t _rgba[4]) : Color(_rgba[0], _rgba[1], _rgba[2], _rgba[3]) {}

            /// Construct Color from unmultiplied RGBA array.
            Color(std::array<uint8_t, 4> _rgba) : Color(_rgba[0], _rgba[1], _rgba[2], _rgba[3]) {}

            /// Construct Color from an RGB array, with alpha set to 255.
            Color(std::array<uint8_t, 3> _rgb) : Color(_rgb[0], _rgb[1], _rgb[2]) {}

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace components
} // namespace rerun
