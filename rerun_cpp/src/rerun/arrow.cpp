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
} // namespace rerun
