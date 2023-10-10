#include "recording_stream.hpp"
#include "components/instance_key.hpp"

#include <rerun.h>

#include <arrow/buffer.h>
#include <vector>

namespace rerun {
    static const auto splat_key = components::InstanceKey(std::numeric_limits<uint64_t>::max());

    static rr_store_kind store_kind_to_c(StoreKind store_kind) {
        switch (store_kind) {
            case StoreKind::Recording:
                return RERUN_STORE_KIND_RECORDING;

            case StoreKind::Blueprint:
                return RERUN_STORE_KIND_BLUEPRINT;
        }

        // This should never happen since if we missed a switch case we'll get a warning on
        // compilers which compiles as an error on CI. But let's play it safe regardless and default
        // to recording.
        return RERUN_STORE_KIND_RECORDING;
    }

    RecordingStream::RecordingStream(const char* app_id, StoreKind store_kind)
        : _store_kind(store_kind) {
        rr_store_info store_info;
        store_info.application_id = app_id;
        store_info.store_kind = store_kind_to_c(store_kind);

        rr_error status = {};
        this->_id = rr_recording_stream_new(&store_info, &status);
        Error(status).log_on_failure();
    }

    RecordingStream::RecordingStream(RecordingStream&& other)
        : _id(other._id), _store_kind(other._store_kind) {
        // Set to `RERUN_REC_STREAM_CURRENT_RECORDING` since it's a no-op on destruction.
        other._id = RERUN_REC_STREAM_CURRENT_RECORDING;
    }

    RecordingStream::~RecordingStream() {
        // C-Api already specifies that the current constants are not destroyed, but we repeat this
        // here, since we rely on this invariant in the move constructor.
        if (_id != RERUN_REC_STREAM_CURRENT_RECORDING &&
            _id != RERUN_REC_STREAM_CURRENT_BLUEPRINT) {
            rr_recording_stream_free(this->_id);
        }
    }

    void RecordingStream::set_global() {
        rr_recording_stream_set_global(_id, store_kind_to_c(_store_kind));
    }

    void RecordingStream::set_thread_local() {
        rr_recording_stream_set_thread_local(_id, store_kind_to_c(_store_kind));
    }

    RecordingStream& RecordingStream::current(StoreKind store_kind) {
        switch (store_kind) {
            case StoreKind::Blueprint: {
                static RecordingStream current_blueprint(
                    RERUN_REC_STREAM_CURRENT_BLUEPRINT,
                    StoreKind::Blueprint
                );
                return current_blueprint;
            }
            case StoreKind::Recording:
            default: {
                static RecordingStream current_recording(
                    RERUN_REC_STREAM_CURRENT_RECORDING,
                    StoreKind::Recording
                );
                return current_recording;
            }
        }
    }

    Error RecordingStream::connect(const char* tcp_addr, float flush_timeout_sec) {
        rr_error status = {};
        rr_recording_stream_connect(_id, tcp_addr, flush_timeout_sec, &status);
        return status;
    }

    Error RecordingStream::save(const char* path) {
        rr_error status = {};
        rr_recording_stream_save(_id, path, &status);
        return status;
    }

    void RecordingStream::flush_blocking() {
        rr_recording_stream_flush_blocking(_id);
    }

    Error RecordingStream::try_log_serialized_batches(
        const char* entity_path, size_t num_instances,
        const std::vector<SerializedComponentBatch>& batches
    ) {
        if (num_instances == 0) {
            return Error::ok();
        }

        std::vector<DataCell> instanced;
        std::vector<DataCell> splatted;

        for (const auto& batch : batches) {
            if (num_instances > 1 && batch.num_instances == 1) {
                splatted.push_back(batch.data_cell);
            } else {
                instanced.push_back(batch.data_cell);
            }
        }

        if (!splatted.empty()) {
            splatted.push_back(components::InstanceKey::to_data_cell(&splat_key, 1).value);
            auto result = try_log_data_row(entity_path, 1, splatted.size(), splatted.data());
            if (result.is_err()) {
                return result;
            }
        }

        return try_log_data_row(entity_path, num_instances, instanced.size(), instanced.data());
    }

    Error RecordingStream::try_log_data_row(
        const char* entity_path, size_t num_instances, size_t num_data_cells,
        const DataCell* data_cells
    ) {
        // Map to C API:
        std::vector<rr_data_cell> c_data_cells(num_data_cells);
        for (size_t i = 0; i < num_data_cells; i++) {
            if (data_cells[i].buffer == nullptr) {
                return Error(
                    ErrorCode::UnexpectedNullArgument,
                    "DataCell buffer is null for cell " + std::to_string(i)
                );
            }

            c_data_cells[i].component_name = data_cells[i].component_name;
            c_data_cells[i].num_bytes = static_cast<uint64_t>(data_cells[i].buffer->size());
            c_data_cells[i].bytes = data_cells[i].buffer->data();
        }

        rr_data_row c_data_row;
        c_data_row.entity_path = entity_path,
        c_data_row.num_instances = static_cast<uint32_t>(num_instances);
        c_data_row.num_data_cells = static_cast<uint32_t>(num_data_cells);
        c_data_row.data_cells = c_data_cells.data();

        rr_error status = {};
        rr_log(_id, &c_data_row, true, &status);
        return status;
    }
} // namespace rerun
