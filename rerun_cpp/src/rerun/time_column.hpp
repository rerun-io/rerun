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
        /// Creates a new time column from an array of time points.
        ///
        /// \param timeline The timeline this column belongs to.
        /// \param timepoints The time points.
        /// Depending on the `TimeType` of the timeline this may be either timestamps or sequence numbers.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        TimeColumn(
            Timeline timeline, Collection<int64_t> timepoints,
            SortingStatus sorting_status = SortingStatus::Sorted
        );

        /// Creates a sequence time column from an array of sequence points.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param sequence_points The sequence points.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the sequence points.
        static TimeColumn from_sequence_points(
            std::string_view timeline_name, Collection<int64_t> sequence_points,
            SortingStatus sorting_status = SortingStatus::Sorted
        ) {
            return TimeColumn(
                Timeline(timeline_name, TimeType::Sequence),
                sequence_points,
                sorting_status
            );
        }

        /// Creates a sequence time column from an array of sequence points.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param timepoints_in_nanoseconds The time points in nanoseconds.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        static TimeColumn from_times_nanoseconds(
            std::string_view timeline_name, Collection<int64_t> timepoints_in_nanoseconds,
            SortingStatus sorting_status = SortingStatus::Sorted
        ) {
            return TimeColumn(
                Timeline(timeline_name, TimeType::Time),
                timepoints_in_nanoseconds,
                sorting_status
            );
        }

        /// Creates a sequence time column from an array of arbitrary std::chrono durations.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param chrono_timepoints The time points as chrono durations.
        /// Make sure the sorting status is correctly specified.
        /// \param sorting_status The sorting status of the time points.
        template <typename TRep, typename TPeriod>
        static TimeColumn from_times(
            std::string_view timeline_name,
            const Collection<std::chrono::duration<TRep, TPeriod>>& chrono_timepoints,
            SortingStatus sorting_status = SortingStatus::Sorted
        ) {
            std::vector<int64_t> timepoints(chrono_timepoints.size());
            for (size_t i = 0; i < chrono_timepoints.size(); i++) {
                timepoints[i] =
                    std::chrono::duration_cast<std::chrono::nanoseconds>(chrono_timepoints[i])
                        .count();
            }
            return TimeColumn(
                Timeline(timeline_name, TimeType::Time),
                std::move(timepoints),
                sorting_status
            );
        }

        /// Creates a sequence time column from a range of sequence points.
        ///
        /// \param timeline_name The name of the timeline this column belongs to.
        /// \param min The minimum sequence point, must be less than `max`.
        /// \param max The maximum sequence point, must be greater than `min`.
        /// \param step The step size between sequence points. Must be non-zero..
        static TimeColumn from_sequence_range(
            std::string_view timeline_name, int64_t min, int64_t max, int64_t step = 1
        ) {
            assert(step > 0);
            assert(min < max);

            auto size = (max - min) / step;
            std::vector<int64_t> sequence_points(static_cast<size_t>(size));
            for (int64_t i = 0; i < size; ++i) {
                sequence_points[static_cast<size_t>(i)] = min + i * step;
            }

            return TimeColumn(
                Timeline(timeline_name, TimeType::Sequence),
                std::move(sequence_points),
                SortingStatus::Sorted
            );
        }

        /// To rerun C API component batch.
        ///
        /// The resulting `rr_time_column` keeps the `arrow::Array` alive until it is released.
        Error to_c_ffi_struct(rr_time_column& out_column) const;
    };
} // namespace rerun
