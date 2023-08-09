#include "color.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct ColorExt : public Color {
            ColorExt(uint32_t _rgba) : Color(_rgba) {}

#define Color ColorExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Color from unmultiplied RGBA values.
            Color(uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255)
                : Color(static_cast<uint32_t>((r << 24) | (g << 16) | (b << 8) | a)) {}

            /// Construct Color from unmultiplied RGBA values.
            Color(const uint8_t (&_rgba)[4]) : Color(_rgba[0], _rgba[1], _rgba[2], _rgba[3]) {}

            /// Construct Color from RGB values, setting alpha to 255.
            Color(const uint8_t (&_rgb)[3]) : Color(_rgb[0], _rgb[1], _rgb[2]) {}

            uint8_t r() const {
                return (rgba >> 24) & 0xFF;
            }

            uint8_t g() const {
                return (rgba >> 16) & 0xFF;
            }

            uint8_t b() const {
                return (rgba >> 8) & 0xFF;
            }

            uint8_t a() const {
                return rgba & 0xFF;
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace datatypes
} // namespace rerun
