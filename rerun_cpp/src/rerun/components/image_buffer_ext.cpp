#include "image_buffer.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct ImageBufferExt {
            rerun::datatypes::Blob buffer;
#define ImageBuffer ImageBufferExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Number of bytes
            size_t size() const {
                return buffer.size();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
