#include "media_type.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct MediaTypeExt {
#define MediaType MediaTypeExt

            // [CODEGEN COPY TO HEADER START]

            MediaType(const char* media_type) : value(media_type) {}

            /// `text/plain`
            static MediaType plain_text() {
                return "text/plain";
            }

            /// `text/markdown`
            static MediaType markdown() {
                return "text/markdown";
            }

            // [CODEGEN COPY TO HEADER END]
        }
    };
#endif
} // namespace components
} // namespace rerun
