#include "component_column.hpp"

#include "arrow_utils.hpp"
#include "c/rerun.h"
#include "compiler_utils.hpp"

#include <arrow/array/array_nested.h>
#include <arrow/buffer.h>
#include <arrow/c/bridge.h>

namespace rerun {
    std::shared_ptr<arrow::Buffer> detail::offsets_from_lengths(const Collection<uint32_t>& lengths
    ) {
        std::vector<uint32_t> offsets(lengths.size() + 1);
        // Some GCC versions see ghosts here - this can't be a null dereference since we have at least 1 element.
        RR_PUSH_WARNINGS
        RR_DISABLE_NULL_DEREF_WARNING
        offsets[0] = 0;
        RR_POP_WARNINGS

        for (size_t i = 0; i < lengths.size(); i++) {
            offsets[i + 1] = offsets[i] + lengths[i];
        }

        return arrow_buffer_from_vector(std::move(offsets));
    }

    Result<ComponentColumn> ComponentColumn::from_batch_with_lengths(
        ComponentBatch batch, const Collection<uint32_t>& lengths
    ) {
        return from_batch_with_offsets(
            std::move(batch),
            detail::offsets_from_lengths(lengths),
            lengths.size()
        );
    }

    Result<ComponentColumn> ComponentColumn::from_batch_with_offsets(
        ComponentBatch batch, Collection<uint32_t> offsets
    ) {
        auto length = offsets.size() - 1;
        auto offset_buffer = arrow_buffer_from_vector(std::move(offsets).to_vector());
        return from_batch_with_offsets(std::move(batch), std::move(offset_buffer), length);
    }

    Result<ComponentColumn> ComponentColumn::from_batch_with_offsets(
        ComponentBatch batch, std::shared_ptr<arrow::Buffer> offsets, size_t num_rows
    ) {
        auto datatype = arrow::list(batch.array->type());
        auto list_array = std::make_shared<arrow::ListArray>(
            std::move(datatype),
            static_cast<int64_t>(num_rows),
            std::move(offsets),
            std::move(batch.array)
        );

        ComponentColumn component_batch;
        component_batch.array = list_array;
        component_batch.component_type = batch.component_type;
        return component_batch;
    }

    Error ComponentColumn::to_c_ffi_struct(rr_component_column& out_component_batch) const {
        if (array == nullptr) {
            return Error(ErrorCode::UnexpectedNullArgument, "array is null");
        }

        out_component_batch.component_type = component_type;
        return arrow::ExportArray(*array, &out_component_batch.array, nullptr);
    }
} // namespace rerun
