#include "data_cell.hpp"
#include "string_utils.hpp"

#include <arrow/api.h>
#include <arrow/c/bridge.h>

#include "c/rerun.h"

namespace rerun {
    Result<rerun::DataCell> DataCell::create_indicator_component(std::string_view archetype_name) {
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto builder = std::make_shared<arrow::NullBuilder>(pool);
        ARROW_RETURN_NOT_OK(builder->AppendNulls(1));
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        DataCell cell;
        cell.num_instances = 1;
        cell.component_name = archetype_name;
        cell.array = std::move(array);
        return cell;
    }

    Error DataCell::to_c_ffi_struct(rr_data_cell& out_cell) const {
        out_cell.component_name = detail::to_rr_string(component_name);
        return arrow::ExportArray(*array, &out_cell.array, &out_cell.schema);
    }
} // namespace rerun
