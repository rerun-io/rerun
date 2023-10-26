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

        bool is_enabled() const {
            return _enabled;
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
        // Methods for controlling time.

        /// Set the current time of the recording, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_sequence("frame_nr", frame_nr)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.remove_timeline`.
        /// @see set_timepoint, set_time_seconds, set_time_nanos, reset_time, remove_timeline
        void set_time_sequence(const char* timeline_name, int64_t sequence_nr);

        /// Set the current time of the recording, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_seconds("sim_time", sim_time_secs)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.remove_timeline`.
        /// @see set_timepoint, set_time_sequence, set_time_nanos, reset_time, remove_timeline
        void set_time_seconds(const char* timeline_name, double seconds);

        /// Set the current time of the recording, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_nanos("sim_time", sim_time_nanos)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.remove_timeline`.
        /// @see set_timepoint, set_time_sequence, set_time_seconds, reset_time, remove_timeline
        void set_time_nanos(const char* timeline_name, int64_t nanos);

        /// Stops logging to the specified timeline for subsequent log calls.
        ///
        /// The timeline is still there, but it will not be updated with any new data.
        ///
        /// No-op if the timeline doesn't exist.
        ///
        /// @see set_timepoint, set_time_sequence, set_time_seconds, reset_time, remove_timeline
        void disable_timeline(const char* timeline_name);

        /// Clears out the current time of the recording, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.reset_time()`.
        /// @see set_timepoint, set_time_sequence, set_time_seconds, set_time_nanos, remove_timeline
        void reset_time();

        // -----------------------------------------------------------------------------------------
        // Methods for logging.

        /// Logs one or more archetype and/or component batches.
        ///
        /// Failures are handled with `Error::handle`.
        ///
        /// @param archetypes_or_component_batches
        /// Any type for which the `AsComponents<T>` trait is implemented.
        /// By default this is any archetype or std::vector/std::array/C-array of components.
        ///
        /// @see try_log
        template <typename... Ts>
        void log(const char* entity_path, const Ts&... archetypes_or_component_batches) {
            if (!is_enabled()) {
                return;
            }
            try_log_with_timeless(entity_path, false, archetypes_or_component_batches...).handle();
        }

        /// Logs one or more archetype and/or component batches as timeless data.
        ///
        /// Timeless data is present on all timelines and behaves as if it was recorded infinitely
        /// far into the past. All timestamp data associated with this message will be dropped right
        /// before sending it to Rerun.
        ///
        /// Failures are handled with `Error::handle`.
        ///
        /// @param archetypes_or_component_batches
        /// Any type for which the `AsComponents<T>` trait is implemented.
        /// By default this is any archetype or std::vector/std::array/C-array of components.
        ///
        /// @see try_log
        template <typename... Ts>
        void log_timeless(const char* entity_path, const Ts&... archetypes_or_component_batches) {
            if (!is_enabled()) {
                return;
            }
            try_log_with_timeless(entity_path, true, archetypes_or_component_batches...).handle();
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
            if (!is_enabled()) {
                return Error::ok();
            }
            return try_log_with_timeless(entity_path, false, archetypes_or_component_batches...);
        }

        /// Logs one or more archetype and/or component batches as timeless data.
        ///
        /// Timeless data is present on all timelines and behaves as if it was recorded infinitely
        /// far into the past. All timestamp data associated with this message will be dropped right
        /// before sending it to Rerun.
        ///
        /// Returns an error if an error occurs during serialization or logging.
        ///
        /// @param archetypes_or_component_batches
        /// Any type for which the `AsComponents<T>` trait is implemented.
        /// By default this is any archetype or std::vector/std::array/C-array of components.
        ///
        /// @see try_log
        template <typename... Ts>
        Error try_log_timeless(
            const char* entity_path, const Ts&... archetypes_or_component_batches
        ) {
            if (!is_enabled()) {
                return Error::ok();
            }
            return try_log_with_timeless(entity_path, true, archetypes_or_component_batches...);
        }

        /// Logs one or more archetype and/or component batches optionally timeless.
        ///
        /// Returns an error if an error occurs during serialization or logging.
        ///
        /// @param archetypes_or_component_batches
        /// Any type for which the `AsComponents<T>` trait is implemented.
        /// By default this is any archetype or std::vector/std::array/C-array of components.
        ///
        /// @see log, try_log, log_timeless, try_log_timeless
        template <typename... Ts>
        Error try_log_with_timeless(
            const char* entity_path, bool timeless, const Ts&... archetypes_or_component_batches
        ) {
            if (!is_enabled()) {
                return Error::ok();
            }
            std::vector<SerializedComponentBatch> serialized_batches;
            Error err;
            (
                [&] {
                    if (err.is_err()) {
                        return;
                    }

                    const Result<std::vector<SerializedComponentBatch>> serialization_result =
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

            return try_log_serialized_batches(entity_path, timeless, serialized_batches);
        }

        /// Logs several serialized batches batches, returning an error on failure.
        ///
        /// The number of instances in each batch must either be equal to the maximum or:
        /// - zero instances - implies a clear
        /// - single instance (but other instances have more) - causes a splat
        Error try_log_serialized_batches(
            const char* entity_path, bool timeless,
            const std::vector<SerializedComponentBatch>& batches
        );

        /// Low level API that logs raw data cells to the recording stream.
        ///
        /// @param num_instances
        /// Each cell is expected to hold exactly `num_instances` instances.
        ///
        /// @param inject_time
        /// If set to `true`, the row's timestamp data will be overridden using the recording
        /// streams internal clock.
        Error try_log_data_row(
            const char* entity_path, size_t num_instances, size_t num_data_cells,
            const DataCell* data_cells, bool inject_time
        );

      private:
        RecordingStream(uint32_t id, StoreKind store_kind);

        uint32_t _id;
        StoreKind _store_kind;
        bool _enabled;
    };
} // namespace rerun
