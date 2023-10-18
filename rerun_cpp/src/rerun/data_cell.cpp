#include "data_cell.hpp"
#include "arrow.hpp"

#include <arrow/api.h>

namespace rerun {

    Result<DataCell> DataCell::create(
        const char* name, const std::shared_ptr<arrow::DataType>& datatype,
        std::shared_ptr<arrow::Array> array
    ) {
        // TODO(andreas): This should be lazily created once just like datatypes are right now, saving repeated allocations.
        auto schema = arrow::schema({arrow::field(name, datatype, false)});

        const auto ipc_result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
        RR_RETURN_NOT_OK(ipc_result.error);

        rerun::DataCell cell;
        cell.component_name = name;
        cell.buffer = std::move(ipc_result.value);

        return cell;
    }

    Result<rerun::DataCell> DataCell::create_indicator_component(const char* indicator_fqname) {
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto builder = std::make_shared<arrow::NullBuilder>(pool);
        ARROW_RETURN_NOT_OK(builder->AppendNulls(1));
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        return create(indicator_fqname, arrow::null(), std::move(array));
    }
} // namespace rerun
