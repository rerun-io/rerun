#include "utf8.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct Utf8Ext {
            std::string value;
#define Utf8 Utf8Ext

            // [CODEGEN COPY TO HEADER START]

            /// Construct a `Utf8` from null-terminated UTF-8.
            Utf8(const char* str) : value(str) {}

            const char* c_str() const {
                return value.c_str();
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace datatypes
} // namespace rerun
