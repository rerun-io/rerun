#include "text_log_level.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../rerun_sdk_export.hpp"

// </CODEGEN_COPY_TO_HEADER>

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
            RERUN_SDK_EXPORT static const TextLogLevel Critical;

            /// Designates very serious errors.
            RERUN_SDK_EXPORT static const TextLogLevel Error;

            /// Designates hazardous situations.
            RERUN_SDK_EXPORT static const TextLogLevel Warning;

            /// Designates useful information.
            RERUN_SDK_EXPORT static const TextLogLevel Info;

            /// Designates lower priority information.
            RERUN_SDK_EXPORT static const TextLogLevel Debug;

            /// Designates very low priority, often extremely verbose, information.
            RERUN_SDK_EXPORT static const TextLogLevel Trace;

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

        const TextLogLevel TextLogLevel::Critical = "CRITICAL";
        const TextLogLevel TextLogLevel::Error = "ERROR";
        const TextLogLevel TextLogLevel::Warning = "WARN";
        const TextLogLevel TextLogLevel::Info = "INFO";
        const TextLogLevel TextLogLevel::Debug = "DEBUG";
        const TextLogLevel TextLogLevel::Trace = "TRACE";
    } // namespace components
} // namespace rerun
