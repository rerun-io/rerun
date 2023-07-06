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

    RecordingStream RecordingStream::s_global;

    RecordingStream::RecordingStream(const char* app_id, const char* addr, StoreKind store_kind) {
        ERROR_CONTEXT("RecordingStream", "");

        int32_t c_store_kind;
        switch (store_kind) {
            case StoreKind::Recording:
                c_store_kind = RERUN_STORE_KIND_RECORDING;
                break;
            case StoreKind::Blueprint:
                c_store_kind = RERUN_STORE_KIND_BLUEPRINT;
                break;
        }

        rr_store_info store_info = {
            .application_id = app_id,
            .store_kind = c_store_kind,
        };
        this->_id = rr_recording_stream_new(&store_info, addr);
    }

    RecordingStream::~RecordingStream() {
        rr_recording_stream_free(this->_id);
    }

    void RecordingStream::init_global(const char* app_id, const char* addr) {
        s_global = RecordingStream{app_id, addr};
    }

    RecordingStream RecordingStream::global() {
        return s_global;
    }

    void RecordingStream::log_data_row(const char* entity_path, uint32_t num_instances,
                                       size_t num_data_cells, const DataCell* data_cells) {
        // Map to C API:
        std::vector<rr_data_cell> c_data_cells;
        c_data_cells.reserve(num_data_cells);
        for (size_t i = 0; i < num_data_cells; ++i) {
            c_data_cells.push_back({
                .component_name = data_cells[i].component_name,
                .num_bytes = data_cells[i].num_bytes,
                .bytes = data_cells[i].bytes,
            });
        }

        const rr_data_row c_data_row = {
            .entity_path = entity_path,
            .num_instances = num_instances,
            .num_data_cells = static_cast<uint32_t>(num_data_cells),
            .data_cells = c_data_cells.data(),
        };

        rr_log(this->_id, &c_data_row);
    }

    // ------------------------------------------------------------------------

    arrow::Result<std::shared_ptr<arrow::Table>> points3(size_t num_points, const float* xyz) {
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        auto x_builder = std::make_shared<arrow::FloatBuilder>(pool);
        auto y_builder = std::make_shared<arrow::FloatBuilder>(pool);
        auto z_builder = std::make_shared<arrow::FloatBuilder>(pool);

        auto nullable = false;

        auto data_type = arrow::struct_({field("x", arrow::float32(), nullable),
                                         field("y", arrow::float32(), nullable),
                                         field("z", arrow::float32(), nullable)});
        auto struct_builder =
            arrow::StructBuilder(data_type, pool, {x_builder, y_builder, z_builder});

        for (size_t i = 0; i < num_points; ++i) {
            ARROW_RETURN_NOT_OK(struct_builder.Append());
            ARROW_RETURN_NOT_OK(x_builder->Append(xyz[3 * i + 0]));
            ARROW_RETURN_NOT_OK(y_builder->Append(xyz[3 * i + 1]));
            ARROW_RETURN_NOT_OK(z_builder->Append(xyz[3 * i + 2]));
        }

        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(struct_builder.Finish(&array));

        auto name = "points"; // Unused, but should be the name of the field in the archetype
        auto schema = arrow::schema({arrow::field(name, data_type, nullable)});

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
