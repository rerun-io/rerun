#include "arrow.hpp"

#include <arrow/api.h>
#include <arrow/io/api.h>
#include <arrow/ipc/api.h>
#include <string>

namespace rerun {
    Result<std::shared_ptr<arrow::Buffer>> ipc_from_table(const arrow::Table& table) {
        ARROW_ASSIGN_OR_RAISE(auto output, arrow::io::BufferOutputStream::Create());
        ARROW_ASSIGN_OR_RAISE(auto writer, arrow::ipc::MakeStreamWriter(output, table.schema()));
        ARROW_RETURN_NOT_OK(writer->WriteTable(table));
        ARROW_RETURN_NOT_OK(writer->Close());

        auto result = output->Finish();
        if (result.ok()) {
            return result.ValueOrDie();
        } else {
            return result.status();
        }
    }

    Result<rerun::DataCell> create_indicator_component(const char* indicator_fqname) {
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto builder = std::make_shared<arrow::NullBuilder>(pool);
        ARROW_RETURN_NOT_OK(builder->AppendNulls(1));
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        auto schema = arrow::schema({arrow::field(indicator_fqname, arrow::null(), false)});

        rerun::DataCell cell;
        cell.component_name = indicator_fqname;
        const auto ipc_result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
        RR_RETURN_NOT_OK(ipc_result.error);
        cell.buffer = std::move(ipc_result.value);

        return cell;
    }
} // namespace rerun
