#include "component_column.hpp"

#include "arrow_utils.hpp"
#include "c/rerun.h"

#include <arrow/array/array_nested.h>
#include <arrow/buffer.h>
#include <arrow/c/bridge.h>

namespace rerun {
    std::shared_ptr<arrow::DataType> ComponentColumn::list_array_type_for(
        std::shared_ptr<arrow::DataType> inner_type
    ) {
        return arrow::list(inner_type);
    }

    Result<ComponentColumn> ComponentColumn::from_batch_with_lengths(
        ComponentBatch batch, const Collection<uint32_t>& lengths,
        std::shared_ptr<arrow::DataType> list_array_type
    ) {
        // Convert lengths into offsets.
        std::vector<uint32_t> offsets(lengths.size() + 1);
        offsets[0] = 0;
        for (size_t i = 0; i < lengths.size(); i++) {
            offsets[i + 1] = offsets[i] + lengths[i];
        }

        return ComponentColumn::from_batch_with_offsets(batch, std::move(offsets), list_array_type);
    }

    Result<ComponentColumn> ComponentColumn::from_batch_with_offsets(
        ComponentBatch batch, Collection<uint32_t> offsets,
        std::shared_ptr<arrow::DataType> list_array_type
    ) {
        auto length = offsets.size() - 1;
        auto offset_buffer = arrow_buffer_from_vector(std::move(offsets).to_vector());
        auto list_array = std::make_shared<arrow::ListArray>(
            list_array_type,
            length,
            offset_buffer,
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
