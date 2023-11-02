#include "text_log_level.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct TextLogLevelExt {
            std::string value;
#define TextLogLevel TextLogLevelExt

            // Don't provide a string_view constructor, std::string constructor exists and covers this.

            // <CODEGEN_COPY_TO_HEADER>

            /// Designates catastrophic failures.
            static const TextLogLevel CRITICAL;

            /// Designates very serious errors.
            static const TextLogLevel ERROR;

            /// Designates hazardous situations.
            static const TextLogLevel WARN;

            /// Designates useful information.
            static const TextLogLevel INFO;

            /// Designates lower priority information.
            static const TextLogLevel DEBUG;

            /// Designates very low priority, often extremely verbose, information.
            static const TextLogLevel TRACE;

            /// Construct `TextLogLevel` from a null-terminated UTF8 string.
            TextLogLevel(const char* str) : value(str) {}

            const char* c_str() const {
                return value.c_str();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };

#undef TextLogLevel
#else
#define TextLogLevelExt TextLogLevel
#endif

        const TextLogLevel TextLogLevel::CRITICAL = "CRITICAL";
        const TextLogLevel TextLogLevel::ERROR = "ERROR";
        const TextLogLevel TextLogLevel::WARN = "WARN";
        const TextLogLevel TextLogLevel::INFO = "INFO";
        const TextLogLevel TextLogLevel::DEBUG = "DEBUG";
        const TextLogLevel TextLogLevel::TRACE = "TRACE";
    } // namespace components
} // namespace rerun
