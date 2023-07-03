// The Rerun C++ SDK.

#ifndef RERUN_HPP
#define RERUN_HPP

#include <rerun.h>

namespace rerun {
    inline const char* version_string() {
        return rerun_version_string();
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
    arrow::Result<std::shared_ptr<arrow::Table>> dummy_table() {
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        arrow::Int64Builder values_builder(pool);
        ARROW_RETURN_NOT_OK(values_builder.Append(1));
        ARROW_RETURN_NOT_OK(values_builder.Append(2));
        ARROW_RETURN_NOT_OK(values_builder.Append(3));
        std::shared_ptr<arrow::Int64Array> array;
        ARROW_RETURN_NOT_OK(values_builder.Finish(&array));

        std::vector<std::shared_ptr<arrow::Field>> fields = {
            arrow::field("values", arrow::int64())};
        auto schema = std::make_shared<arrow::Schema>(fields);
        return arrow::Table::Make(schema, {array});
    }

    arrow::Result<std::shared_ptr<arrow::Table>> points3(size_t num_points,
                                                         const float* xyz) {
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        auto x_builder = std::make_shared<arrow::FloatBuilder>(pool);
        auto y_builder = std::make_shared<arrow::FloatBuilder>(pool);
        auto z_builder = std::make_shared<arrow::FloatBuilder>(pool);

        auto data_type = arrow::struct_({field("x", arrow::float32()),
                                         field("y", arrow::float32()),
                                         field("z", arrow::float32())});
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

        auto nullable = false;
        auto schema =
            arrow::schema({arrow::field("Point3DType", data_type, nullable)});

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

    arrow::Result<std::shared_ptr<arrow::Buffer>> create_buffer() {
        ARROW_ASSIGN_OR_RAISE(auto table, dummy_table());
        return ipc_from_table(*table);
    }
} // namespace rerun

#endif // RERUN_WITH_ARROW

// ----------------------------------------------------------------------------

#endif // RERUN_HPP
