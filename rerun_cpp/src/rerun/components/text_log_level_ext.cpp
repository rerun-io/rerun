#include "text_log_level.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct TextLogLevelExt {
            std::string value;
#define TextLogLevel TextLogLevelExt

            // [CODEGEN COPY TO HEADER START]

            /// Designates catastrophic failures.
            static const TextLogLevel CRITICAL = "CRITICAL";

            /// Designates very serious errors.
            static const TextLogLevel ERROR = "ERROR";

            /// Designates hazardous situations.
            static const TextLogLevel WARN = "WARN";

            /// Designates useful information.
            static const TextLogLevel INFO = "INFO";

            /// Designates lower priority information.
            static const TextLogLevel DEBUG = "DEBUG";

            /// Designates very low priority, often extremely verbose, information.
            static const TextLogLevel TRACE = "TRACE";

            /// Construct `TextLogLevel` from a zero-terminated UTF8 string.
            TextLogLevel(const char* str) : value(str) {}

            const char* c_str() const {
                return value.c_str();
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace components
} // namespace rerun
