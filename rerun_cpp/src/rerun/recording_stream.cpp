#include "recording_stream.hpp"
#include "c/rerun.h"
#include "component_batch.hpp"
#include "config.hpp"
#include "sdk_info.hpp"
#include "string_utils.hpp"

#include <arrow/buffer.h>

#include <string> // to_string
#include <vector>

namespace rerun {
    static rr_store_kind store_kind_to_c(StoreKind store_kind) {
        switch (store_kind) {
            case StoreKind::Recording:
                return RR_STORE_KIND_RECORDING;

            case StoreKind::Blueprint:
                return RR_STORE_KIND_BLUEPRINT;
        }

        // This should never happen since if we missed a switch case we'll get a warning on
        // compilers which compiles as an error on CI. But let's play it safe regardless and default
        // to recording.
        return RR_STORE_KIND_RECORDING;
    }

    RecordingStream::RecordingStream(
        std::string_view app_id, std::string_view recording_id, StoreKind store_kind
    )
        : _store_kind(store_kind) {
        check_binary_and_header_version_match().handle();

        rr_store_info store_info;
        store_info.application_id = detail::to_rr_string(app_id);
        store_info.recording_id = detail::to_rr_string(recording_id);
        store_info.store_kind = store_kind_to_c(store_kind);

        rr_error status = {};
        this->_id = rr_recording_stream_new(&store_info, is_default_enabled(), &status);
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
        // Set to `RR_REC_STREAM_CURRENT_RECORDING` since it's a no-op on destruction.
        other._id = RR_REC_STREAM_CURRENT_RECORDING;
    }

    RecordingStream::RecordingStream(uint32_t id, StoreKind store_kind)
        : _id(id), _store_kind(store_kind) {
        check_binary_and_header_version_match().handle();

        rr_error status = {};
        this->_enabled = rr_recording_stream_is_enabled(this->_id, &status);
        Error(status).handle();
    }

    RecordingStream::~RecordingStream() {
        // C-Api already specifies that the current constants are not destroyed, but we repeat this
        // here, since we rely on this invariant in the move constructor.
        if (_id != RR_REC_STREAM_CURRENT_RECORDING && _id != RR_REC_STREAM_CURRENT_BLUEPRINT) {
            rr_recording_stream_free(this->_id);
        }
    }

    void RecordingStream::set_global() const {
        rr_recording_stream_set_global(_id, store_kind_to_c(_store_kind));
    }

    void RecordingStream::set_thread_local() const {
        rr_recording_stream_set_thread_local(_id, store_kind_to_c(_store_kind));
    }

    RecordingStream& RecordingStream::current(StoreKind store_kind) {
        switch (store_kind) {
            case StoreKind::Blueprint: {
                static RecordingStream current_blueprint(
                    RR_REC_STREAM_CURRENT_BLUEPRINT,
                    StoreKind::Blueprint
                );
                return current_blueprint;
            }
            case StoreKind::Recording:
            default: {
                static RecordingStream current_recording(
                    RR_REC_STREAM_CURRENT_RECORDING,
                    StoreKind::Recording
                );
                return current_recording;
            }
        }
    }

    Error RecordingStream::connect(std::string_view tcp_addr, float flush_timeout_sec) const {
        rr_error status = {};
        rr_recording_stream_connect(
            _id,
            detail::to_rr_string(tcp_addr),
            flush_timeout_sec,
            &status
        );
        return status;
    }

    Error RecordingStream::spawn(const SpawnOptions& options, float flush_timeout_sec) const {
        rr_spawn_options rerun_c_options = {};
        options.fill_rerun_c_struct(rerun_c_options);
        rr_error status = {};
        rr_recording_stream_spawn(_id, &rerun_c_options, flush_timeout_sec, &status);
        return status;
    }

    Error RecordingStream::save(std::string_view path) const {
        rr_error status = {};
        rr_recording_stream_save(_id, detail::to_rr_string(path), &status);
        return status;
    }

    Error RecordingStream::to_stdout() const {
        rr_error status = {};
        rr_recording_stream_stdout(_id, &status);
        return status;
    }

    void RecordingStream::flush_blocking() const {
        rr_recording_stream_flush_blocking(_id);
    }

    void RecordingStream::set_time_sequence(std::string_view timeline_name, int64_t sequence_nr)
        const {
        if (!is_enabled()) {
            return;
        }
        rr_error status = {};
        rr_recording_stream_set_time_sequence(
            _id,
            detail::to_rr_string(timeline_name),
            sequence_nr,
            &status
        );
        Error(status).handle(); // Too unlikely to fail to make it worth forwarding.
    }

    void RecordingStream::set_time_seconds(std::string_view timeline_name, double seconds) const {
        if (!is_enabled()) {
            return;
        }
        rr_error status = {};
        rr_recording_stream_set_time_seconds(
            _id,
            detail::to_rr_string(timeline_name),
            seconds,
            &status
        );
        Error(status).handle(); // Too unlikely to fail to make it worth forwarding.
    }

    void RecordingStream::set_time_nanos(std::string_view timeline_name, int64_t nanos) const {
        rr_error status = {};
        rr_recording_stream_set_time_nanos(
            _id,
            detail::to_rr_string(timeline_name),
            nanos,
            &status
        );
        Error(status).handle(); // Too unlikely to fail to make it worth forwarding.
    }

    void RecordingStream::disable_timeline(std::string_view timeline_name) const {
        rr_error status = {};
        rr_recording_stream_disable_timeline(_id, detail::to_rr_string(timeline_name), &status);
        Error(status).handle(); // Too unlikely to fail to make it worth forwarding.
    }

    void RecordingStream::reset_time() const {
        rr_recording_stream_reset_time(_id);
    }

    Error RecordingStream::try_log_serialized_batches(
        std::string_view entity_path, bool static_, std::vector<ComponentBatch> batches
    ) const {
        if (!is_enabled()) {
            return Error::ok();
        }

        std::vector<ComponentBatch> instanced;

        for (const auto& batch : batches) {
            instanced.push_back(std::move(batch));
        }

        bool inject_time = !static_;

        return try_log_data_row(entity_path, instanced.size(), instanced.data(), inject_time);
    }

    Error RecordingStream::try_log_data_row(
        std::string_view entity_path, size_t num_component_batches,
        const ComponentBatch* component_batches, bool inject_time
    ) const {
        if (!is_enabled()) {
            return Error::ok();
        }
        // Map to C API:
        std::vector<rr_component_batch> c_component_batches(num_component_batches);
        for (size_t i = 0; i < num_component_batches; i++) {
            RR_RETURN_NOT_OK(component_batches[i].to_c_ffi_struct(c_component_batches[i]));
        }

        rr_data_row c_data_row;
        c_data_row.entity_path = detail::to_rr_string(entity_path);
        c_data_row.num_component_batches = static_cast<uint32_t>(num_component_batches);
        c_data_row.component_batches = c_component_batches.data();

        rr_error status = {};
        rr_recording_stream_log(_id, c_data_row, inject_time, &status);

        return status;
    }

    Error RecordingStream::try_log_file_from_path(
        const std::filesystem::path& filepath, std::string_view entity_path_prefix, bool static_
    ) const {
        if (!is_enabled()) {
            return Error::ok();
        }

        rr_error status = {};
        rr_recording_stream_log_file_from_path(
            _id,
            detail::to_rr_string(filepath.string()),
            detail::to_rr_string(entity_path_prefix),
            static_,
            &status
        );

        return status;
    }

    Error RecordingStream::try_log_file_from_contents(
        const std::filesystem::path& filepath, const std::byte* contents, size_t contents_size,
        std::string_view entity_path_prefix, bool static_
    ) const {
        if (!is_enabled()) {
            return Error::ok();
        }

        rr_bytes data = {};
        data.bytes = reinterpret_cast<const uint8_t*>(contents);
        data.length = static_cast<uint32_t>(contents_size);

        rr_error status = {};
        rr_recording_stream_log_file_from_contents(
            _id,
            detail::to_rr_string(filepath.string()),
            data,
            detail::to_rr_string(entity_path_prefix),
            static_,
            &status
        );

        return status;
    }

    Error RecordingStream::try_send_columns(
        std::string_view entity_path, rerun::Collection<TimeColumn> time_columns,
        rerun::Collection<ComponentColumn> component_columns
    ) const {
        if (!is_enabled()) {
            return Error::ok();
        }

        std::vector<rr_time_column> c_time_columns;
        c_time_columns.reserve(time_columns.size());
        for (const auto& time_column : time_columns) {
            rr_time_column c_time_column;
            RR_RETURN_NOT_OK(time_column.to_c_ffi_struct(c_time_column));
            c_time_columns.push_back(c_time_column);
        }

        std::vector<rr_component_column> c_component_columns;
        c_component_columns.reserve(component_columns.size());
        for (const auto& component_batch : component_columns) {
            rr_component_column c_component_batch;
            RR_RETURN_NOT_OK(component_batch.to_c_ffi_struct(c_component_batch));
            c_component_columns.push_back(c_component_batch);
        }

        rr_error status = {};
        rr_recording_stream_send_columns(
            _id,
            detail::to_rr_string(entity_path),
            c_time_columns.data(),
            static_cast<uint32_t>(c_time_columns.size()),
            c_component_columns.data(),
            static_cast<uint32_t>(c_component_columns.size()),
            &status
        );

        return status;
    }

} // namespace rerun
