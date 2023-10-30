// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/text_log.fbs".

#pragma once

#include "../component_batch.hpp"
#include "../components/color.hpp"
#include "../components/text.hpp"
#include "../components/text_log_level.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"
#include "../util.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// **Archetype**: A log entry in a text log, comprised of a text body and its log level.
        ///
        /// ## Example
        ///
        /// ### `text_log_integration`:
        /// ```cpp,ignore
        /// #include <loguru.hpp>
        /// #include <rerun.hpp>
        ///
        /// void loguru_to_rerun(void* user_data, const loguru::Message& message) {
        ///     // NOTE: `rerun::RecordingStream` is thread-safe.
        ///     const rerun::RecordingStream* rec = reinterpret_cast<const rerun::RecordingStream*>(user_data);
        ///
        ///     rec->log("loguru", rerun::TextLog(message.message));
        /// }
        ///
        /// int main() {
        ///     const auto rec = rerun::RecordingStream("rerun_example_text_log");
        ///     rec.spawn().throw_on_failure();
        ///
        ///     // Log a text entry directly:
        ///     rec.log(
        ///         "log",
        ///         rerun::TextLog("this entry has loglevel TRACE").with_level(rerun::TextLogLevel::TRACE)
        ///     );
        ///
        ///     loguru::add_callback(
        ///         "rerun",
        ///         loguru_to_rerun,
        ///         const_cast<void*>(reinterpret_cast<const void*>(&rec)),
        ///         loguru::Verbosity_INFO
        ///     );
        ///
        ///     LOG_F(INFO, "This INFO log got added through the standard logging interface");
        ///
        ///     loguru::remove_callback("rerun"); // we need to do this before `rec` goes out of scope
        /// }
        /// ```
        struct TextLog {
            /// The body of the message.
            rerun::components::Text text;

            /// The verbosity level of the message.
            ///
            /// This can be used to filter the log messages in the Rerun Viewer.
            std::optional<rerun::components::TextLogLevel> level;

            /// Optional color to use for the log line in the Rerun Viewer.
            std::optional<rerun::components::Color> color;

            /// Name of the indicator component, used to identify the archetype when converting to a list of components.
            static const char INDICATOR_COMPONENT_NAME[];
            /// Indicator component, used to identify the archetype when converting to a list of components.
            using IndicatorComponent = components::IndicatorComponent<INDICATOR_COMPONENT_NAME>;

          public:
            TextLog() = default;
            TextLog(TextLog&& other) = default;

            explicit TextLog(rerun::components::Text _text) : text(std::move(_text)) {}

            /// The verbosity level of the message.
            ///
            /// This can be used to filter the log messages in the Rerun Viewer.
            TextLog with_level(rerun::components::TextLogLevel _level) && {
                level = std::move(_level);
                // See: https://github.com/rerun-io/rerun/issues/4027
                WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
            }

            /// Optional color to use for the log line in the Rerun Viewer.
            TextLog with_color(rerun::components::Color _color) && {
                color = std::move(_color);
                // See: https://github.com/rerun-io/rerun/issues/4027
                WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }
        };

    } // namespace archetypes

    template <typename T>
    struct AsComponents;

    template <>
    struct AsComponents<archetypes::TextLog> {
        /// Serialize all set component batches.
        static Result<std::vector<SerializedComponentBatch>> serialize(
            const archetypes::TextLog& archetype
        );
    };
} // namespace rerun
