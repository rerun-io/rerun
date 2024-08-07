#pragma once

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
        // TODO: docs
        TimeColumn(Timeline timeline, Collection<int64_t> timepoints, SortingStatus sorting_status);

        /// Creates a new time column from an array of sequence points.
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

        // TODO: implement this
        /// Creates a new time column from an array of sequence points.
        // static TimeColumn from_seconds(
        //     std::string_view timeline_name, const Collection<float>& time_in_seconds,
        //     SortingStatus sorting_status = SortingStatus::Sorted,
        // );

        // TODO: std::chrono support, range support.

        /// To rerun C API component batch.
        ///
        /// The resulting `rr_time_column` keeps the `arrow::Array` alive until it is released.
        Error to_c_ffi_struct(rr_time_column& out_column) const;
    };
} // namespace rerun
