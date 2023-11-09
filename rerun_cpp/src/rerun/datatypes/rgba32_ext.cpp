#include "rgba32.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct Rgba32Ext : public Rgba32 {
            Rgba32Ext(uint32_t _rgba) : Rgba32(_rgba) {}

#define Rgba32 Rgba32Ext

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct Rgba32 from unmultiplied RGBA values.
            Rgba32(uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255)
                : Rgba32(static_cast<uint32_t>((r << 24) | (g << 16) | (b << 8) | a)) {}

            /// Construct Rgba32 from unmultiplied RGBA values.
            Rgba32(const uint8_t (&_rgba)[4]) : Rgba32(_rgba[0], _rgba[1], _rgba[2], _rgba[3]) {}

            /// Construct Rgba32 from RGB values, setting alpha to 255.
            Rgba32(const uint8_t (&_rgb)[3]) : Rgba32(_rgb[0], _rgb[1], _rgb[2]) {}

            uint8_t r() const {
                return static_cast<uint8_t>((rgba >> 24) & 0xFF);
            }

            uint8_t g() const {
                return static_cast<uint8_t>((rgba >> 16) & 0xFF);
            }

            uint8_t b() const {
                return static_cast<uint8_t>((rgba >> 8) & 0xFF);
            }

            uint8_t a() const {
                return static_cast<uint8_t>(rgba & 0xFF);
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace datatypes
} // namespace rerun
