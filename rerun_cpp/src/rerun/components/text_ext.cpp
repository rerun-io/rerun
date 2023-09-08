#include "text.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct TextExt {
            std::string value;
#define Text TextExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct `Text` from a zero-terminated UTF8 string.
            Text(const char* str) : value(str) {}

            const char* c_str() const {
                return value.c_str();
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace components
} // namespace rerun
