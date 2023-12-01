#include "data_cell.hpp"

#include <arrow/c/bridge.h>

#include "c/rerun.h"

namespace rerun {
    Error DataCell::to_c_ffi_struct(rr_data_cell& out_cell) const {
        if (array == nullptr) {
            return Error(ErrorCode::UnexpectedNullArgument, "array is null");
        }

        out_cell.component_type = component_type;
        return arrow::ExportArray(*array, &out_cell.array, nullptr);
    }
} // namespace rerun
