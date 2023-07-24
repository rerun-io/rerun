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

    arrow::Result<std::shared_ptr<arrow::Table>> points3(size_t num_points, const float* xyz) {
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        auto nullable = false;

        ARROW_ASSIGN_OR_RAISE(auto builder, rr::components::Point3D::new_arrow_array_builder(pool));
        ARROW_RETURN_NOT_OK(rr::components::Point3D::fill_arrow_array_builder(
            builder.get(),
            (rr::components::Point3D*)xyz,
            num_points
        ));

        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        auto name = "points"; // Unused, but should be the name of the field in the archetype
        auto schema = arrow::schema(
            {arrow::field(name, rr::components::Point3D::to_arrow_datatype(), nullable)}
        );

        return arrow::Table::Make(schema, {array});
    }

    arrow::Result<std::shared_ptr<arrow::Buffer>> ipc_from_table(const arrow::Table& table) {
        ERROR_CONTEXT("ipc_from_table", "");
        ARROW_ASSIGN_OR_RAISE(auto output, arrow::io::BufferOutputStream::Create());
        ARROW_ASSIGN_OR_RAISE(auto writer, arrow::ipc::MakeStreamWriter(output, table.schema()));
        ARROW_RETURN_NOT_OK(writer->WriteTable(table));
        ARROW_RETURN_NOT_OK(writer->Close());
        return output->Finish();
    }
} // namespace rr
