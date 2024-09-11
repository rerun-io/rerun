#if 0

// <CODEGEN_COPY_TO_HEADER>
#include <chrono>
// </CODEGEN_COPY_TO_HEADER>

#include "../datatypes/video_time_mode.hpp"
#include "video_timestamp.hpp"

namespace rerun {
    namespace components {

        // <CODEGEN_COPY_TO_HEADER>

        /// Creates a new `VideoTimestamp` component.
        /// \param video_time Timestamp value, type defined by `time_mode`.
        /// \param time_mode How to interpret `video_time`.
        VideoTimestamp(int64_t video_time, rerun::datatypes::VideoTimeMode time_mode)
            : VideoTimestamp(rerun::datatypes::VideoTimestamp{video_time, time_mode}) {}

        /// Creates a new `VideoTimestamp` from time since video start.
        /// \param time Time since video start.
        template <typename TRep, typename TPeriod>
        VideoTimestamp(std::chrono::duration<TRep, TPeriod> time)
            : VideoTimestamp(
                  std::chrono::duration_cast<std::chrono::nanoseconds>(time).count(),
                  datatypes::VideoTimeMode::Nanoseconds
              ) {}

        /// Creates a new [`VideoTimestamp`] from seconds since video start.
        static VideoTimestamp from_seconds(double seconds) {
            return VideoTimestamp(std::chrono::duration<double>(seconds));
        }

        /// Creates a new [`VideoTimestamp`] from milliseconds since video start.
        static VideoTimestamp from_milliseconds(double milliseconds) {
            return VideoTimestamp(std::chrono::duration<double, std::milli>(milliseconds));
        }

        /// Creates a new [`VideoTimestamp`] from nanoseconds since video start.
        static VideoTimestamp from_nanoseconds(int64_t nanoseconds) {
            return VideoTimestamp(std::chrono::nanoseconds(nanoseconds));
        }

        // </CODEGEN_COPY_TO_HEADER>

    } // namespace components
} // namespace rerun

#endif
