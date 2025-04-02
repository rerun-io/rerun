#pragma once

#include <cassert>
#include <chrono>
#include <memory> // shared_ptr

#include "collection.hpp"
#include "error.hpp"
#include "timeline.hpp"

// X.h (of X11) has a macro called `Unsorted`
// See <https://codebrowser.dev/kde/include/X11/X.h.html#_M/Unsorted>
// and <https://github.com/rerun-io/rerun/issues/7846>.
#ifdef Unsorted
#error \
    "Found a macro 'Unsorted' (probably from X11), conflicting with `rerun::SortingStatus::Unsorted`. Add '#undef Unsorted' before '#include <rerun.hpp>' to work around this."
#endif

struct rr_time_column;

namespace arrow {
    class Array;
}

namespace rerun {
    /// Describes whether an array is known to be sorted or not.
    enum class SortingStatus {
        /// It's not known whether the array is sorted or not.
        Unknown = 0,

        /// The array is known to be sorted.
        Sorted = 1,

        /// The array is known to be unsorted.
        Unsorted = 2,
    };

    /// Arrow-encoded data for a column of time points.
    ///
    /// \see `rerun::RecordingStream::send_columns`
    struct TimeColumn {
        /// The timeline this column belongs to.
        Timeline timeline;

        /// Time points as a primitive array of i64.
        std::shared_ptr<arrow::Array> array;

        /// The sorting order of the `times` array.
        SortingStatus sorting_status;

      public:
        /// Creates a time column from an array of time points.
        ///
        /// \param timeline The timeline this column belongs to.
        /// \param times The time values.
        /// Depending on the `TimeType` of the timeline this may be either sequence numbers, durations, or timestamps.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        /// Already sorted time points may perform better.
        TimeColumn(
            Timeline timeline, Collection<int64_t> times,
            SortingStatus sorting_status = SortingStatus::Unknown
        );

        /// Creates a time column from an array of sequence points.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param sequence_points The sequence points.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the sequence points.
        /// Already sorted time points may perform better.
        ///
        /// \deprecated Use `from_sequence` instead.
        [[deprecated("Use `from_sequence` instead.")]] static TimeColumn from_sequence_points(
            std::string timeline_name, Collection<int64_t> sequence_points,
            SortingStatus sorting_status = SortingStatus::Unknown
        ) {
            return TimeColumn(
                Timeline(std::move(timeline_name), TimeType::Sequence),
                std::move(sequence_points),
                sorting_status
            );
        }

        /// Creates a column from an array of sequence points, e.g. frame numbers.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param sequence_points The sequence points.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the sequence points.
        /// Already sorted time points may perform better.
        static TimeColumn from_sequence(
            std::string timeline_name, Collection<int64_t> sequence_points,
            SortingStatus sorting_status = SortingStatus::Unknown
        ) {
            return TimeColumn(
                Timeline(std::move(timeline_name), TimeType::Sequence),
                std::move(sequence_points),
                sorting_status
            );
        }

        /// Creates a time column from an array of nanoseconds.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param times_in_nanoseconds Time values in nanoseconds.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        /// Already sorted time points may perform better.
        [[deprecated("Use 'from_duration_nanos' or `from_nanos_since_epoch' instead"
        )]] static TimeColumn
            from_nanoseconds(
                std::string timeline_name, Collection<int64_t> times_in_nanoseconds,
                SortingStatus sorting_status = SortingStatus::Unknown
            ) {
            return TimeColumn(
                Timeline(std::move(timeline_name), TimeType::Duration),
                std::move(times_in_nanoseconds),
                sorting_status
            );
        }

        /// Creates a time column from an array of seconds.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param times_in_secs Time values in seconds.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        /// Already sorted time points may perform better.
        [[deprecated("Use 'from_duration_secs' or `from_secs_since_epoch' instead"
        )]] static TimeColumn
            from_seconds(
                std::string timeline_name, Collection<double> times_in_secs,
                SortingStatus sorting_status = SortingStatus::Unknown
            );

        // -----------
        // Durations:

        /// Creates a time column from an array of arbitrary std::chrono durations.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param durations Time values as chrono durations.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        /// Already sorted time points may perform better.
        template <typename TRep, typename TPeriod>
        static TimeColumn from_durations(
            std::string timeline_name,
            const Collection<std::chrono::duration<TRep, TPeriod>>& durations,
            SortingStatus sorting_status = SortingStatus::Unknown
        ) {
            std::vector<int64_t> times(durations.size());
            for (size_t i = 0; i < durations.size(); i++) {
                times[i] =
                    std::chrono::duration_cast<std::chrono::nanoseconds>(durations[i]).count();
            }
            return TimeColumn(
                Timeline(std::move(timeline_name), TimeType::Duration),
                std::move(times),
                sorting_status
            );
        }

        /// \deprecated Use `from_durations` instead.
        template <typename TRep, typename TPeriod>
        [[deprecated("Use `from_durations` instead.")]] static TimeColumn from_times(
            std::string timeline_name,
            const Collection<std::chrono::duration<TRep, TPeriod>>& chrono_times,
            SortingStatus sorting_status = SortingStatus::Unknown
        ) {
            return from_durations<TRep, TPeriod>(timeline_name, chrono_times, sorting_status);
        }

        /// Creates a duration column from an array of nanoseconds.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param duration_in_nanos Duration values in nanoseconds.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        /// Already sorted time points may perform better.
        static TimeColumn from_duration_nanoseconds(
            std::string timeline_name, Collection<int64_t> duration_in_nanos,
            SortingStatus sorting_status = SortingStatus::Unknown
        ) {
            return TimeColumn(
                Timeline(std::move(timeline_name), TimeType::Duration),
                std::move(duration_in_nanos),
                sorting_status
            );
        }

        /// Creates a duration column from an array of seconds.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param duration_in_secs Duration values in seconds.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        /// Already sorted time points may perform better.
        static TimeColumn from_duration_secs(
            std::string timeline_name, Collection<double> duration_in_secs,
            SortingStatus sorting_status = SortingStatus::Unknown
        ) {
            std::vector<int64_t> duration_in_nanos;
            duration_in_nanos.reserve(duration_in_secs.size());
            for (auto time_in_secs : duration_in_secs) {
                duration_in_nanos.push_back(static_cast<int64_t>(time_in_secs * 1.0e9 + 0.5));
            }
            return TimeColumn(
                Timeline(std::move(timeline_name), TimeType::Duration),
                std::move(duration_in_nanos),
                sorting_status
            );
        }

        // -----------
        // Timestamps:

        template <typename TClock>
        static TimeColumn from_time_points(
            std::string timeline_name,
            const Collection<std::chrono::time_point<TClock>>& time_points,
            SortingStatus sorting_status = SortingStatus::Unknown
        ) {
            std::vector<int64_t> nanos_since_epoch;
            nanos_since_epoch.reserve(time_points.size());
            for (auto timepoint : time_points) {
                auto nanos = std::chrono::duration_cast<std::chrono::nanoseconds>(
                    timepoint.time_since_epoch()
                );
                nanos_since_epoch.push_back(nanos.count());
            }
            return TimeColumn::from_nanos_since_epoch(
                std::move(timeline_name),
                nanos_since_epoch,
                sorting_status
            );
        }

        /// Creates a timestamp column from an array of nanoseconds since Unix Epoch (1970-01-01 00:00:00 UTC).
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param timestamp_in_nanos Timestamp values in nanoseconds.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        /// Already sorted time points may perform better.
        static TimeColumn from_nanos_since_epoch(
            std::string timeline_name, Collection<int64_t> timestamp_in_nanos,
            SortingStatus sorting_status = SortingStatus::Unknown
        ) {
            return TimeColumn(
                Timeline(std::move(timeline_name), TimeType::Timestamp),
                std::move(timestamp_in_nanos),
                sorting_status
            );
        }

        /// Creates a duration column from an array of seconds since Unix Epoch (1970-01-01 00:00:00 UTC).
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param timestamp_in_secs Timestamp values in seconds.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        /// Already sorted time points may perform better.
        static TimeColumn from_secs_since_epoch(
            std::string timeline_name, Collection<double> timestamp_in_secs,
            SortingStatus sorting_status = SortingStatus::Unknown
        ) {
            std::vector<int64_t> timestamp_in_nanos;
            timestamp_in_nanos.reserve(timestamp_in_secs.size());
            for (auto time_in_secs : timestamp_in_secs) {
                timestamp_in_nanos.push_back(static_cast<int64_t>(time_in_secs * 1.0e9 + 0.5));
            }
            return TimeColumn::from_nanos_since_epoch(
                std::move(timeline_name),
                std::move(timestamp_in_nanos),
                sorting_status
            );
        }

        // -----------------------------------------------------------------------------

        /// To rerun C API component batch.
        ///
        /// The resulting `rr_time_column` keeps the `arrow::Array` alive until it is released.
        Error to_c_ffi_struct(rr_time_column& out_column) const;
    };
} // namespace rerun
