#include "recording_stream.hpp"
#include "components/instance_key.hpp"

#include "c/rerun.h"

#include <arrow/buffer.h>
#include <vector>

static rr_string to_rr_string(std::string_view str) {
    rr_string result;
    result.utf8 = str.data();
    result.length_in_bytes = static_cast<uint32_t>(str.length());
    return result;
}

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

    RecordingStream::RecordingStream(std::string_view app_id, StoreKind store_kind)
        : _store_kind(store_kind) {
        rr_store_info store_info;
        store_info.application_id = to_rr_string(app_id);
        store_info.store_kind = store_kind_to_c(store_kind);

        rr_error status = {};
        this->_id = rr_recording_stream_new(&store_info, &status);
        auto err = Error(status);
        if (err.is_ok()) {
            this->_enabled = rr_recording_stream_is_enabled(this->_id, &status);
            Error(status).handle();
        } else {
            this->_enabled = false;
            err.handle();
        }
    }

    RecordingStream::RecordingStream(RecordingStream&& other)
        : _id(other._id), _store_kind(other._store_kind), _enabled(other._enabled) {
        // Set to `RERUN_REC_STREAM_CURRENT_RECORDING` since it's a no-op on destruction.
        other._id = RERUN_REC_STREAM_CURRENT_RECORDING;
    }

    RecordingStream::RecordingStream(uint32_t id, StoreKind store_kind)
        : _id(id), _store_kind(store_kind) {
        rr_error status = {};
        this->_enabled = rr_recording_stream_is_enabled(this->_id, &status);
        Error(status).handle();
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

    Error RecordingStream::connect(std::string_view tcp_addr, float flush_timeout_sec) {
        rr_error status = {};
        rr_recording_stream_connect(_id, to_rr_string(tcp_addr), flush_timeout_sec, &status);
        return status;
    }

    Error RecordingStream::spawn(
        uint16_t port, const char* memory_limit, const char* executable_name,
        const char* executable_path, float flush_timeout_sec
    ) {
        rr_error status = {};
        rr_spawn_options spawn_opts = {
            port = port,
            memory_limit = memory_limit,
            executable_name = executable_name,
            executable_path = executable_path,
        };
        rr_recording_stream_spawn(_id, &spawn_opts, flush_timeout_sec, &status);
        return status;
    }

    Error RecordingStream::save(std::string_view path) {
        rr_error status = {};
        rr_recording_stream_save(_id, to_rr_string(path.data()), &status);
        return status;
    }

    void RecordingStream::flush_blocking() {
        rr_recording_stream_flush_blocking(_id);
    }

    void RecordingStream::set_time_sequence(std::string_view timeline_name, int64_t sequence_nr) {
        if (!is_enabled()) {
            return;
        }
        rr_error status = {};
        rr_recording_stream_set_time_sequence(
            _id,
            to_rr_string(timeline_name),
            sequence_nr,
            &status
        );
        Error(status).handle(); // Too unlikely to fail to make it worth forwarding.
    }

    void RecordingStream::set_time_seconds(std::string_view timeline_name, double seconds) {
        if (!is_enabled()) {
            return;
        }
        rr_error status = {};
        rr_recording_stream_set_time_seconds(_id, to_rr_string(timeline_name), seconds, &status);
        Error(status).handle(); // Too unlikely to fail to make it worth forwarding.
    }

    void RecordingStream::set_time_nanos(std::string_view timeline_name, int64_t nanos) {
        rr_error status = {};
        rr_recording_stream_set_time_nanos(_id, to_rr_string(timeline_name), nanos, &status);
        Error(status).handle(); // Too unlikely to fail to make it worth forwarding.
    }

    void RecordingStream::disable_timeline(std::string_view timeline_name) {
        rr_error status = {};
        rr_recording_stream_disable_timeline(_id, to_rr_string(timeline_name), &status);
        Error(status).handle(); // Too unlikely to fail to make it worth forwarding.
    }

    void RecordingStream::reset_time() {
        rr_recording_stream_reset_time(_id);
    }

    Error RecordingStream::try_log_serialized_batches(
        std::string_view entity_path, bool timeless,
        const std::vector<SerializedComponentBatch>& batches
    ) {
        if (!is_enabled()) {
            return Error::ok();
        }
        size_t num_instances_max = 0;
        for (const auto& batch : batches) {
            num_instances_max = std::max(num_instances_max, batch.num_instances);
        }

        std::vector<DataCell> instanced;
        std::vector<DataCell> splatted;

        for (const auto& batch : batches) {
            if (num_instances_max > 1 && batch.num_instances == 1) {
                splatted.push_back(batch.data_cell);
            } else {
                instanced.push_back(batch.data_cell);
            }
        }

        bool inject_time = !timeless;

        if (!splatted.empty()) {
            splatted.push_back(components::InstanceKey::to_data_cell(&splat_key, 1).value);
            auto result =
                try_log_data_row(entity_path, 1, splatted.size(), splatted.data(), inject_time);
            if (result.is_err()) {
                return result;
            }
        }

        return try_log_data_row(
            entity_path,
            num_instances_max,
            instanced.size(),
            instanced.data(),
            inject_time
        );
    }

    Error RecordingStream::try_log_data_row(
        std::string_view entity_path, size_t num_instances, size_t num_data_cells,
        const DataCell* data_cells, bool inject_time
    ) {
        if (!is_enabled()) {
            return Error::ok();
        }
        // Map to C API:
        std::vector<rr_data_cell> c_data_cells(num_data_cells);
        for (size_t i = 0; i < num_data_cells; i++) {
            if (data_cells[i].buffer == nullptr) {
                return Error(
                    ErrorCode::UnexpectedNullArgument,
                    "DataCell buffer is null for cell " + std::to_string(i)
                );
            }

            c_data_cells[i].component_name = to_rr_string(data_cells[i].component_name);
            c_data_cells[i].num_bytes = static_cast<uint64_t>(data_cells[i].buffer->size());
            c_data_cells[i].bytes = data_cells[i].buffer->data();
        }

        rr_data_row c_data_row;
        c_data_row.entity_path = to_rr_string(entity_path);
        c_data_row.num_instances = static_cast<uint32_t>(num_instances);
        c_data_row.num_data_cells = static_cast<uint32_t>(num_data_cells);
        c_data_row.data_cells = c_data_cells.data();

        rr_error status = {};
        rr_recording_stream_log(_id, &c_data_row, inject_time, &status);
        return status;
    }
} // namespace rerun
