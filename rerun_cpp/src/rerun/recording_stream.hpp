#pragma once

#include <cstdint> // uint32_t etc.

#include "component_list.hpp"
#include "data_cell.hpp"
#include "error.hpp"

namespace rerun {
    struct DataCell;

    enum class StoreKind {
        Recording,
        Blueprint,
    };

    /// A `RecordingStream` handles everything related to logging data into Rerun.
    ///
    /// ## Multithreading and ordering
    ///
    /// Internally, all operations are linearized into a pipeline:
    /// - All operations sent by a given thread will take effect in the same exact order as that
    ///   thread originally sent them in, from its point of view.
    /// - There isn't any well defined global order across multiple threads.
    ///
    /// This means that e.g. flushing the pipeline (`flush_blocking`) guarantees that all
    /// previous data sent by the calling thread has been recorded; no more, no less.
    /// (e.g. it does not mean that all file caches are flushed)
    ///
    /// ## Shutdown
    ///
    /// The `RecordingStream` can only be shutdown by dropping all instances of it, at which point
    /// it will automatically take care of flushing any pending data that might remain in the
    /// pipeline.
    ///
    /// TODO(andreas): The only way of having two instances of a `RecordingStream` is currently to
    /// set it as a the global.
    ///
    /// Shutting down cannot ever block.
    class RecordingStream {
      public:
        /// Creates a new recording stream to log to.
        /// @param app_id The user-chosen name of the application doing the logging.
        RecordingStream(const char* app_id, StoreKind store_kind = StoreKind::Recording);
        ~RecordingStream();

        RecordingStream(RecordingStream&& other);

        // TODO(andreas): We could easily make the recording stream trivial to copy by bumping Rusts
        // ref counter by adding a copy of the recording stream to the list of C recording streams.
        // Doing it this way would likely yield the most consistent behavior when interacting with
        // global streams (and especially when interacting with different languages in the same
        // application).
        RecordingStream(const RecordingStream&) = delete;
        RecordingStream() = delete;

        // -----------------------------------------------------------------------------------------
        // Properties

        StoreKind kind() const {
            return _store_kind;
        }

        // -----------------------------------------------------------------------------------------
        // Controlling globally available instances of RecordingStream.

        /// Replaces the currently active recording for this stream's store kind in the global scope
        /// with this one.
        ///
        /// Afterwards, destroying this recording stream will *not* change the global recording
        /// stream, as it increases an internal ref-count.
        void set_global();

        /// Replaces the currently active recording for this stream's store kind in the thread-local
        /// scope with this one
        ///
        /// Afterwards, destroying this recording stream will *not* change the thread local
        /// recording stream, as it increases an internal ref-count.
        void set_thread_local();

        /// Retrieves the most appropriate globally available recording stream for the given kind.
        ///
        /// I.e. thread-local first, then global.
        /// If neither was set, any operations on the returned stream will be no-ops.
        static RecordingStream& current(StoreKind store_kind = StoreKind::Recording);

        // -----------------------------------------------------------------------------------------
        // Directing the recording stream. Either of these needs to be called, otherwise the stream
        // will buffer up indefinitely.

        /// Connect to a remote Rerun Viewer on the given ip:port.
        ///
        /// Requires that you first start a Rerun Viewer by typing 'rerun' in a terminal.
        ///
        /// flush_timeout_sec:
        /// The minimum time the SDK will wait during a flush before potentially
        /// dropping data if progress is not being made. Passing a negative value indicates no
        /// timeout, and can cause a call to `flush` to block indefinitely.
        ///
        /// This function returns immediately.
        Error connect(const char* tcp_addr = "127.0.0.1:9876", float flush_timeout_sec = 2.0);

        /// Stream all log-data to a given file.
        ///
        /// This function returns immediately.
        Error save(const char* path);

        /// Initiates a flush the batching pipeline and waits for it to propagate.
        ///
        /// See `RecordingStream` docs for ordering semantics and multithreading guarantees.
        void flush_blocking();

        // -----------------------------------------------------------------------------------------
        // Methods for logging.

        /// Logs an archetype.
        ///
        /// Prefer this interface for ease of use over the more general `log_components` interface.
        ///
        /// Alias for `log_archetype`.
        /// TODO(andreas): Would be nice if this were able to combine both log_archetype and
        /// log_components!
        ///
        /// Logs any failure via `Error::log_on_failure`
        template <typename T>
        void log(const char* entity_path, const T& archetype) {
            log_archetype(entity_path, archetype);
        }

        /// Logs an archetype.
        ///
        /// Prefer this interface for ease of use over the more general `log_components` interface.
        ///
        /// Logs any failure via `Error::log_on_failure`
        template <typename T>
        void log_archetype(const char* entity_path, const T& archetype) {
            try_log_archetype(entity_path, archetype).log_on_failure();
        }

        /// Logs a an archetype, returning an error on failure.
        ///
        /// @see log_archetype
        template <typename T>
        Error try_log_archetype(const char* entity_path, const T& archetype) {
            const auto data_cells_result = archetype.to_data_cells();
            if (data_cells_result.is_ok()) {
                return try_log_data_row(
                    entity_path,
                    archetype.num_instances(),
                    data_cells_result.value.size(),
                    data_cells_result.value.data()
                );
            } else {
                return data_cells_result.error;
            }
        }

        /// Logs a list of component arrays.
        ///
        /// This forms the "medium level API", for easy to use high level api, prefer `log` to log
        /// built-in archetypes.
        ///
        /// Expects component arrays as std::vector, std::array or C arrays.
        ///
        /// TODO(andreas): More documentation, examples etc.
        ///
        /// Logs any failure via `Error::log_on_failure`
        template <typename... Ts>
        void log_components(const char* entity_path, const Ts&... component_array) {
            try_log_components(entity_path, component_array...).log_on_failure();
        }

        /// Logs a list of component arrays, returning an error on failure.
        ///
        /// @see log_components
        template <typename... Ts>
        Error try_log_components(const char* entity_path, const Ts&... component_array) {
            // TODO(andreas): Handle splats.
            const size_t num_instances = size_of_first_collection(component_array...);

            std::vector<DataCell> data_cells;
            data_cells.reserve(sizeof...(Ts));
            const auto error = push_data_cells(data_cells, component_array...);
            if (error.is_err()) {
                return error;
            }

            return try_log_data_row(
                entity_path,
                num_instances,
                data_cells.size(),
                data_cells.data()
            );
        }

        /// Low level API that logs raw data cells to the recording stream.
        ///
        /// I.e. logs a number of components arrays (each with a same number of instances) to a
        /// single entity path.
        Error try_log_data_row(
            const char* entity_path, size_t num_instances, size_t num_data_cells,
            const DataCell* data_cells
        );

      private:
        template <typename C, typename... Ts>
        static size_t size_of_first_collection(const C& first, const Ts&...) {
            return ComponentList(first).size;
        }

        template <typename C, typename... Ts>
        static Error push_data_cells(
            std::vector<DataCell>& data_cells, const C& first, const Ts&... rest
        ) {
            const auto cell_result = ComponentList(first).to_data_cell();
            if (cell_result.is_err()) {
                return cell_result.error;
            }
            data_cells.push_back(cell_result.value);
            return push_data_cells(data_cells, rest...);
        }

        static Error push_data_cells(std::vector<DataCell>&) {
            return Error();
        }

        RecordingStream(uint32_t id, StoreKind store_kind) : _id(id), _store_kind(store_kind) {}

        uint32_t _id;
        StoreKind _store_kind;
    };
} // namespace rerun
