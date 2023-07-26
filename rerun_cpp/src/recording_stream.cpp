#include "recording_stream.hpp"

#include <rerun.h>

#include <loguru.hpp>
#include <vector>

namespace rr {
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

    void RecordingStream::log_data_row(
        const char* entity_path, uint32_t num_instances, size_t num_data_cells,
        const DataCell* data_cells
    ) {
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
} // namespace rr
