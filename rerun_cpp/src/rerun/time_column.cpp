#include "time_column.hpp"

#include "arrow_utils.hpp"
#include "c/rerun.h"

#include <arrow/array/array_base.h>
#include <arrow/array/util.h>
#include <arrow/buffer.h>
#include <arrow/c/bridge.h>

namespace rerun {
    TimeColumn::TimeColumn(
        Timeline timeline_, rerun::Collection<int64_t> times, SortingStatus sorting_status_
    )
        : timeline(timeline_), sorting_status(sorting_status_) {
        static auto datatype = arrow::int64();

        // We could alternatively assume that the `times` collection stays alive long enough.
        // To do so we would store it on this struct. But what if the collection itself is already a borrow?
        // This would add more complicated constrains, so instead we take ownership of the data here
        // which may or may not be another copy (if the collection already owns the data this is just a move).
        auto length = static_cast<int64_t>(times.size());
        auto buffer = arrow_buffer_from_vector(std::move(times).to_vector());
        auto buffers = std::vector<std::shared_ptr<arrow::Buffer>>{nullptr, buffer};
        auto array_data = std::make_shared<arrow::ArrayData>(datatype, length, std::move(buffers));
        array = arrow::MakeArray(array_data);
    }

    TimeColumn TimeColumn::from_seconds(
        std::string timeline_name, Collection<double> times_in_secs, SortingStatus sorting_status
    ) {
        std::vector<int64_t> times_in_nanoseconds;
        times_in_nanoseconds.reserve(times_in_secs.size());
        for (auto time_in_secs : times_in_secs) {
            times_in_nanoseconds.push_back(static_cast<int64_t>(time_in_secs * 1.0e9 + 0.5));
        }
        return TimeColumn(
            Timeline(std::move(timeline_name), TimeType::Duration),
            std::move(times_in_nanoseconds),
            sorting_status
        );
    }

    Error to_rr_sorting_status(SortingStatus status, rr_sorting_status& out_status) {
        switch (status) {
            case SortingStatus::Unknown:
                out_status = RR_SORTING_STATUS_UNKNOWN;
                break;
            case SortingStatus::Sorted:
                out_status = RR_SORTING_STATUS_SORTED;
                break;
            case SortingStatus::Unsorted:
                out_status = RR_SORTING_STATUS_UNSORTED;
                break;
            default:
                return Error(ErrorCode::InvalidEnumValue, "Invalid SortingStatus");
        }
        return Error::ok();
    }

    Error TimeColumn::to_c_ffi_struct(rr_time_column& out_column) const {
        RR_RETURN_NOT_OK(timeline.to_c_ffi_struct(out_column.timeline));
        RR_RETURN_NOT_OK(arrow::ExportArray(*array, &out_column.array, nullptr));
        RR_RETURN_NOT_OK(to_rr_sorting_status(sorting_status, out_column.sorting_status));
        return Error::ok();
    }

} // namespace rerun
