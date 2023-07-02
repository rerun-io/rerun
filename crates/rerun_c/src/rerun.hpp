// The Rerun C++ SDK.

#ifndef RERUN_HPP
#define RERUN_HPP

namespace rerun_c {
#include <rerun.h>
}

namespace rerun {
inline const char* version_string() { return rerun_c::rerun_version_string(); }
}  // namespace rerun

// ----------------------------------------------------------------------------
// Arrow integration

#if RERUN_WITH_ARROW

#include <arrow/api.h>
#include <arrow/io/api.h>
#include <arrow/ipc/api.h>

arrow::Result<std::shared_ptr<arrow::Table>> MakeTable() {
    arrow::MemoryPool* pool = arrow::default_memory_pool();
    arrow::Int64Builder values_builder(pool);
    ARROW_RETURN_NOT_OK(values_builder.Append(1));
    ARROW_RETURN_NOT_OK(values_builder.Append(2));
    ARROW_RETURN_NOT_OK(values_builder.Append(3));
    std::shared_ptr<arrow::Int64Array> arr;
    ARROW_RETURN_NOT_OK(values_builder.Finish(&arr));

    std::vector<std::shared_ptr<arrow::Field>> fields = {
        arrow::field("values", arrow::int64())};
    auto schema = std::make_shared<arrow::Schema>(fields);
    return arrow::Table::Make(schema, {arr});
}

namespace rerun {
arrow::Result<std::shared_ptr<arrow::Buffer>> ipc_from_table(
    const arrow::Table& table) {
    ARROW_ASSIGN_OR_RAISE(auto output, arrow::io::BufferOutputStream::Create());
    ARROW_ASSIGN_OR_RAISE(auto writer,
                          arrow::ipc::MakeStreamWriter(output, table.schema()));
    ARROW_RETURN_NOT_OK(writer->WriteTable(table));
    ARROW_RETURN_NOT_OK(writer->Close());
    return output->Finish();
}

arrow::Result<std::shared_ptr<arrow::Buffer>> create_buffer() {
    ARROW_ASSIGN_OR_RAISE(auto table, MakeTable());
    return ipc_from_table(*table);
}
}  // namespace rerun

#endif  // RERUN_WITH_ARROW

// ----------------------------------------------------------------------------

#endif  // RERUN_HPP
