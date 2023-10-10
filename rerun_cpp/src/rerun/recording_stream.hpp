#pragma once

#include <cstdint> // uint32_t etc.
#include <vector>

#include "component_batch.hpp"
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
    ///
    /// ## Logging
    ///
    /// Internally, the stream will automatically micro-batch multiple log calls to optimize
    /// transport.
    /// See [SDK Micro Batching](https://www.rerun.io/docs/reference/sdk-micro-batching) for
    /// more information.
    ///
    /// The data will be timestamped automatically based on the `RecordingStream`'s
    /// internal clock.
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
        /// Prefer this interface for ease of use over the more general `log_component_batches`
        /// interface.
        ///
        /// Logs any failure via `Error::log_on_failure`
        template <typename T>
        void log(const char* entity_path, const T& archetype) {
            log_archetype(entity_path, archetype);
        }

        /// @copydoc log
        template <typename T>
        void log_archetype(const char* entity_path, const T& archetype) {
            try_log_archetype(entity_path, archetype).log_on_failure();
        }

        /// Logs a an archetype, returning an error on failure.
        ///
        /// @see log_archetype
        template <typename T>
        Error try_log_archetype(const char* entity_path, const T& archetype) {
            auto serialization_result = archetype.serialize();
            RR_RETURN_NOT_OK(serialization_result.error);
            return try_log_serialized_batches(
                entity_path,
                archetype.num_instances(),
                serialization_result.value
            );
        }

        /// Logs a single component batch.
        ///
        /// This forms the "medium level API", for easy to use high level api, prefer `log` to log
        /// built-in archetypes.
        ///
        /// Logs any failure via `Error::log_on_failure`
        ///
        /// @param component_batch
        /// Expects component batch as std::vector, std::array or C arrays.
        ///
        /// @see try_log_component_batch, log_component_batches, try_log_component_batches
        template <typename T>
        void log_component_batch(const char* entity_path, const T& component_batch) {
            const auto serialized = AsComponents<T>().serialize(component_batch);
            serialized.error.log_on_failure();
            try_log_serialized_batches(entity_path, serialized.value.size(), {serialized.value})
                .log_on_failure();
        }

        /// Logs a single component batch.
        ///
        /// This forms the "medium level API", for easy to use high level api, prefer `log` to log
        /// built-in archetypes.
        ///
        /// @param component_batch
        /// Expects component batch as std::vector, std::array or C arrays.
        ///
        /// @see log_component_batch, log_component_batches, try_log_component_batches
        template <typename T>
        Error try_log_component_batch(const char* entity_path, const T& component_batch) {
            const auto serialized = AsComponents<T>().serialize(component_batch);
            RR_RETURN_NOT_OK(serialized.error);
            return try_log_serialized_batches(
                entity_path,
                serialized.value.size(),
                {serialized.value}
            );
        }

        /// Logs several component batches.
        ///
        /// This forms the "medium level API", for easy to use high level api, prefer `log` to log
        /// built-in archetypes.
        ///
        /// Logs any failure via `Error::log_on_failure`
        ///
        /// @param component_batches
        /// Expects component batches as std::vector, std::array or C arrays.
        ///
        /// @param num_instances
        /// Specify the expected number of component instances present in each
        /// list. Each can have either:
        /// - exactly `num_instances` instances,
        /// - a single instance (splat),
        /// - or zero instance (clear).
        ///
        /// @see try_log_component_batches
        template <typename... Ts>
        void log_component_batches(
            const char* entity_path, size_t num_instances, const Ts&... component_batches
        ) {
            try_log_component_batches(entity_path, num_instances, component_batches...)
                .log_on_failure();
        }

        /// Logs several component batches, returning an error on failure.
        ///
        ///
        /// @param num_instances
        /// Specify the expected number of component instances present in each
        /// list. Each can have either:
        /// - exactly `num_instances` instances,
        /// - a single instance (splat),
        /// - or zero instance (clear).
        ///
        /// @see log_component_batches
        template <typename... Ts>
        Error try_log_component_batches(
            const char* entity_path, size_t num_instances, const Ts&... component_batches
        ) {
            std::vector<SerializedComponentBatch> serialized_batches;
            serialized_batches.reserve(sizeof...(Ts));
            Error err;

            (
                [&] {
                    const auto serialization_result =
                        AsComponents<Ts>().serialize(component_batches);
                    if (serialization_result.is_err()) {
                        err = serialization_result.error;
                    } else {
                        for (auto& batch : serialization_result.value) {
                            serialized_batches.emplace_back(std::move(batch));
                        }
                    }
                }(),
                ...
            );
            RR_RETURN_NOT_OK(err);

            return try_log_serialized_batches(entity_path, num_instances, serialized_batches);
        }

        // TODO: docs
        Error try_log_serialized_batches(
            const char* entity_path, size_t num_instances,
            const std::vector<SerializedComponentBatch>& batches
        );

        /// Low level API that logs raw data cells to the recording stream.
        ///
        /// @param num_instances
        /// Each cell is expected to hold exactly `num_instances` instances.
        Error try_log_data_row(
            const char* entity_path, size_t num_instances, size_t num_data_cells,
            const DataCell* data_cells
        );

      private:
        RecordingStream(uint32_t id, StoreKind store_kind) : _id(id), _store_kind(store_kind) {}

        uint32_t _id;
        StoreKind _store_kind;
    };
} // namespace rerun
