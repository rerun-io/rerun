#include "data_cell.hpp"
#include "arrow.hpp"
#include "string_utils.hpp"

#include <arrow/api.h>
#include <arrow/c/bridge.h>

namespace rerun {

    Result<DataCell> DataCell::create(
        std::string name_, const std::shared_ptr<arrow::DataType>& datatype_,
        std::shared_ptr<arrow::Array> array_
    ) {
        // // TODO(andreas): This should be lazily created once just like datatypes are right now, saving repeated allocations.
        // auto schema = arrow::schema({arrow::field(name, datatype, false)});

        // const auto ipc_result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
        // RR_RETURN_NOT_OK(ipc_result.error);

        rerun::DataCell cell;
        cell.component_name = std::move(name_);
        cell.datatype = datatype_;
        cell.array = std::move(array_);
        return cell;
    }

    Result<rerun::DataCell> DataCell::create_indicator_component(std::string indicator_fqname) {
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto builder = std::make_shared<arrow::NullBuilder>(pool);
        ARROW_RETURN_NOT_OK(builder->AppendNulls(1));
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        return create(std::move(indicator_fqname), arrow::null(), std::move(array));
    }

    Error DataCell::to_c(rr_data_cell& out_cell) const {
        out_cell.component_name = detail::to_rr_string(component_name);
        return arrow::ExportArray(*array, &out_cell.array, &out_cell.schema);
    }
} // namespace rerun
