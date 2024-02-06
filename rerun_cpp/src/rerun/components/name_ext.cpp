#include "name.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct NameExt {
            std::string value;
#define Name NameExt

            // Don't provide a string_view constructor, std::string constructor exists and covers this.

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct `Name` from a null-terminated UTF8 string.
            Name(const char* str) : value(str) {}

            const char* c_str() const {
                return value.c_str();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
