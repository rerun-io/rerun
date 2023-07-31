#include "rerun.hpp"

#include <arrow/api.h>
#include <arrow/io/api.h>
#include <arrow/ipc/api.h>
#include <rerun.h>

#include <loguru.hpp>

namespace rr {
    const char* version_string() {
        return rr_version_string();
    }

    // ------------------------------------------------------------------------

    arrow::Result<std::shared_ptr<arrow::Buffer>> ipc_from_table(const arrow::Table& table) {
        ERROR_CONTEXT("ipc_from_table", "");
        ARROW_ASSIGN_OR_RAISE(auto output, arrow::io::BufferOutputStream::Create());
        ARROW_ASSIGN_OR_RAISE(auto writer, arrow::ipc::MakeStreamWriter(output, table.schema()));
        ARROW_RETURN_NOT_OK(writer->WriteTable(table));
        ARROW_RETURN_NOT_OK(writer->Close());
        return output->Finish();
    }
} // namespace rr
