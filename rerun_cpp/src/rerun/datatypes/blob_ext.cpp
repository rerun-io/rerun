#include "blob.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct BlobExt {
            rerun::Collection<uint8_t> data;
#define Blob BlobExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Number of bytes
            size_t size() const {
                return data.size();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace datatypes
} // namespace rerun
