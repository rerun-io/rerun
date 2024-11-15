#include "component_batch.hpp"

#include <arrow/c/bridge.h>

#include "c/rerun.h"

namespace rerun {
    Error ComponentBatch::to_c_ffi_struct(rr_component_batch& out_component_batch) const {
        if (array == nullptr) {
            return Error(ErrorCode::UnexpectedNullArgument, "array is null");
        }

        out_component_batch.component_type = component_type;
        return arrow::ExportArray(*array, &out_component_batch.array, nullptr);
    }
} // namespace rerun
