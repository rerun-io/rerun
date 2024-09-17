#pragma once

#include <cassert>
#include <chrono>
#include <memory> // shared_ptr

#include "collection.hpp"
#include "error.hpp"
#include "timeline.hpp"

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
        /// Depending on the `TimeType` of the timeline this may be either timestamps or sequence numbers.
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
        static TimeColumn from_sequence_points(
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
        static TimeColumn from_nanoseconds(
            std::string timeline_name, Collection<int64_t> times_in_nanoseconds,
            SortingStatus sorting_status = SortingStatus::Unknown
        );

        /// Creates a time column from an array of seconds.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param times_in_seconds Time values in seconds.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        /// Already sorted time points may perform better.
        static TimeColumn from_seconds(
            std::string timeline_name, Collection<double> times_in_seconds,
            SortingStatus sorting_status = SortingStatus::Unknown
        );

        /// Creates a time column from an array of arbitrary std::chrono durations.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param chrono_times Time values as chrono durations.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        /// Already sorted time points may perform better.
        template <typename TRep, typename TPeriod>
        static TimeColumn from_times(
            std::string timeline_name,
            const Collection<std::chrono::duration<TRep, TPeriod>>& chrono_times,
            SortingStatus sorting_status = SortingStatus::Unknown
        ) {
            std::vector<int64_t> times(chrono_times.size());
            for (size_t i = 0; i < chrono_times.size(); i++) {
                times[i] =
                    std::chrono::duration_cast<std::chrono::nanoseconds>(chrono_times[i]).count();
            }
            return TimeColumn(
                Timeline(std::move(timeline_name), TimeType::Time),
                std::move(times),
                sorting_status
            );
        }

        /// To rerun C API component batch.
        ///
        /// The resulting `rr_time_column` keeps the `arrow::Array` alive until it is released.
        Error to_c_ffi_struct(rr_time_column& out_column) const;
    };
} // namespace rerun
