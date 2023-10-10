#pragma once

#include <cstdint> // uint32_t etc.
#include <vector>

#include "as_components.hpp"
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

        /// Logs one or more archetype and/or component batches.
        ///
        /// Logs any failure via `Error::log_on_failure`
        ///
        /// @param archetypes_or_component_batches
        /// Any type for which the `AsComponents<T>` trait is implemented.
        /// By default this is any archetype or std::vector/std::array/C-array of components.
        ///
        /// @see try_log
        template <typename... Ts>
        void log(const char* entity_path, const Ts&... archetypes_or_component_batches) {
            try_log(entity_path, archetypes_or_component_batches...).log_on_failure();
        }

        /// Logs one or more archetype and/or component batches.
        ///
        /// Returns an error if an error occurs during serialization or logging.
        ///
        /// @param archetypes_or_component_batches
        /// Any type for which the `AsComponents<T>` trait is implemented.
        /// By default this is any archetype or std::vector/std::array/C-array of components.
        ///
        /// @see try_log
        template <typename... Ts>
        Error try_log(const char* entity_path, const Ts&... archetypes_or_component_batches) {
            std::vector<SerializedComponentBatch> serialized_batches;
            Error err;
            (
                [&] {
                    if (err.is_err()) {
                        return;
                    }

                    const auto serialization_result =
                        AsComponents<Ts>().serialize(archetypes_or_component_batches);
                    if (serialization_result.is_err()) {
                        err = serialization_result.error;
                        return;
                    }

                    if (serialized_batches.empty()) {
                        // Fast path for the first batch (which is usually the only one!)
                        serialized_batches = std::move(serialization_result.value);
                    } else {
                        serialized_batches.insert(
                            serialized_batches.end(),
                            std::make_move_iterator(serialization_result.value.begin()),
                            std::make_move_iterator(serialization_result.value.end())
                        );
                    }
                }(),
                ...
            );
            RR_RETURN_NOT_OK(err);

            return try_log_serialized_batches(entity_path, serialized_batches);
        }

        /// Logs several serialized batches batches, returning an error on failure.
        ///
        /// The number of instances in each batch must either be equal to each other or:
        /// - zero instances - implies a clear
        /// - single instance (but other instances have more) - causes a splat
        Error try_log_serialized_batches(
            const char* entity_path, const std::vector<SerializedComponentBatch>& batches
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
