#if 0

// <CODEGEN_COPY_TO_HEADER>
#include <chrono>
// </CODEGEN_COPY_TO_HEADER>

#include "../datatypes/video_time_mode.hpp"
#include "video_timestamp.hpp"

namespace rerun {
    namespace components {

        // <CODEGEN_COPY_TO_HEADER>

        /// Creates a new `VideoTimestamp` from a presentation timestamp as a chrono duration.
        template <typename TRep, typename TPeriod>
        VideoTimestamp(std::chrono::duration<TRep, TPeriod> time) {
            timestamp.timestamp_ns =
                std::chrono::duration_cast<std::chrono::nanoseconds>(time).count();
        }

        /// Creates a new `VideoTimestamp` from a presentation timestamp in seconds.
        static VideoTimestamp from_seconds(double seconds) {
            return VideoTimestamp(std::chrono::duration<double>(seconds));
        }

        /// Creates a new `VideoTimestamp` from a presentation timestamp in milliseconds.
        static VideoTimestamp from_milliseconds(double milliseconds) {
            return VideoTimestamp(std::chrono::duration<double, std::milli>(milliseconds));
        }

        /// Creates a new `VideoTimestamp` from a presentation timestamp in nanoseconds.
        static VideoTimestamp from_nanoseconds(int64_t nanoseconds) {
            return VideoTimestamp(std::chrono::nanoseconds(nanoseconds));
        }

        // </CODEGEN_COPY_TO_HEADER>

    } // namespace components
} // namespace rerun

#endif
