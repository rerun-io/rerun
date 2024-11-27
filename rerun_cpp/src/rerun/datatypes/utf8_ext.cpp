#include "utf8.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct Utf8Ext {
            std::string value;
#define Utf8 Utf8Ext

            // Don't provide a string_view constructor, std::string constructor exists and covers this.

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct from a C string.
            Utf8(const char* utf8_) : value(utf8_) {}

            /// Explicit copy assignment from a C string to avoid ambiguity in some cases.
            Utf8& operator=(const char* utf8_) {
                value = utf8_;
                return *this;
            }

            /// Returns a pointer to the underlying C string.
            const char* c_str() const {
                return value.c_str();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace datatypes
} // namespace rerun
