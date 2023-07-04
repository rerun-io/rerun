// The Rerun C++ SDK.

#ifndef RERUN_HPP
#define RERUN_HPP

#include <rerun.h>

namespace rerun {
    inline const char* version_string() {
        return rr_version_string();
    }
} // namespace rerun

// ----------------------------------------------------------------------------
// Arrow integration

#if RERUN_WITH_ARROW

#include <arrow/api.h>
#include <arrow/io/api.h>
#include <arrow/ipc/api.h>

#include <loguru.hpp>

namespace rerun {
    arrow::Result<std::shared_ptr<arrow::Table>> points3(size_t num_points,
                                                         const float* xyz) {
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        auto x_builder = std::make_shared<arrow::FloatBuilder>(pool);
        auto y_builder = std::make_shared<arrow::FloatBuilder>(pool);
        auto z_builder = std::make_shared<arrow::FloatBuilder>(pool);

        auto nullable = false;

        auto data_type =
            arrow::struct_({field("x", arrow::float32(), nullable),
                            field("y", arrow::float32(), nullable),
                            field("z", arrow::float32(), nullable)});
        auto struct_builder = arrow::StructBuilder(
            data_type, pool, {x_builder, y_builder, z_builder});

        for (size_t i = 0; i < num_points; ++i) {
            ARROW_RETURN_NOT_OK(struct_builder.Append());
            ARROW_RETURN_NOT_OK(x_builder->Append(xyz[3 * i + 0]));
            ARROW_RETURN_NOT_OK(y_builder->Append(xyz[3 * i + 1]));
            ARROW_RETURN_NOT_OK(z_builder->Append(xyz[3 * i + 2]));
        }

        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(struct_builder.Finish(&array));

        auto name = "Point3DType"; // TODO: I think the name here is unused?
        auto schema = arrow::schema({arrow::field(name, data_type, nullable)});

        return arrow::Table::Make(schema, {array});
    }

    arrow::Result<std::shared_ptr<arrow::Buffer>> ipc_from_table(
        const arrow::Table& table) {
        ERROR_CONTEXT("ipc_from_table", "");
        ARROW_ASSIGN_OR_RAISE(auto output,
                              arrow::io::BufferOutputStream::Create());
        ARROW_ASSIGN_OR_RAISE(
            auto writer, arrow::ipc::MakeStreamWriter(output, table.schema()));
        ARROW_RETURN_NOT_OK(writer->WriteTable(table));
        ARROW_RETURN_NOT_OK(writer->Close());
        return output->Finish();
    }
} // namespace rerun

#endif // RERUN_WITH_ARROW

// ----------------------------------------------------------------------------

#endif // RERUN_HPP
