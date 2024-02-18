#pragma once

#include <chrono>
#include <cstdint> // uint32_t etc.
#include <optional>
#include <string_view>
#include <vector>

#include "as_components.hpp"
#include "collection.hpp"
#include "error.hpp"
#include "spawn_options.hpp"

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
    /// A `RecordingStream` is thread-safe.
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
        ///
        /// \param app_id The user-chosen name of the application doing the logging.
        /// \param recording_id The user-chosen name of the recording being logged to.
        /// \param store_kind Whether to log to the recording store or the blueprint store.
        RecordingStream(
            std::string_view app_id, std::string_view recording_id = std::string_view(),
            StoreKind store_kind = StoreKind::Recording
        );
        ~RecordingStream();

        /// \private
        RecordingStream(RecordingStream&& other);

        // TODO(andreas): We could easily make the recording stream trivial to copy by bumping Rusts
        // ref counter by adding a copy of the recording stream to the list of C recording streams.
        // Doing it this way would likely yield the most consistent behavior when interacting with
        // global streams (and especially when interacting with different languages in the same
        // application).
        /// \private
        RecordingStream(const RecordingStream&) = delete;
        /// \private
        RecordingStream() = delete;

        // -----------------------------------------------------------------------------------------
        /// \name Properties
        /// @{

        /// Returns the store kind as passed during construction
        StoreKind kind() const {
            return _store_kind;
        }

        /// Returns whether the recording stream is enabled.
        ///
        /// All log functions early out if a recording stream is disabled.
        /// Naturally, logging functions that take unserialized data will skip the serialization step as well.
        bool is_enabled() const {
            return _enabled;
        }

        /// @}

        // -----------------------------------------------------------------------------------------
        /// \name Controlling globally available instances of RecordingStream.
        /// @{

        /// Replaces the currently active recording for this stream's store kind in the global scope
        /// with this one.
        ///
        /// Afterwards, destroying this recording stream will *not* change the global recording
        /// stream, as it increases an internal ref-count.
        void set_global() const;

        /// Replaces the currently active recording for this stream's store kind in the thread-local
        /// scope with this one
        ///
        /// Afterwards, destroying this recording stream will *not* change the thread local
        /// recording stream, as it increases an internal ref-count.
        void set_thread_local() const;

        /// Retrieves the most appropriate globally available recording stream for the given kind.
        ///
        /// I.e. thread-local first, then global.
        /// If neither was set, any operations on the returned stream will be no-ops.
        static RecordingStream& current(StoreKind store_kind = StoreKind::Recording);

        /// @}

        // -----------------------------------------------------------------------------------------
        /// \name Directing the recording stream.
        /// \details Either of these needs to be called, otherwise the stream will buffer up indefinitely.
        /// @{

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
        Error connect(std::string_view tcp_addr = "127.0.0.1:9876", float flush_timeout_sec = 2.0)
            const;

        /// Spawns a new Rerun Viewer process from an executable available in PATH, then connects to it
        /// over TCP.
        ///
        /// If a Rerun Viewer is already listening on this TCP port, the stream will be redirected to
        /// that viewer instead of starting a new one.
        ///
        /// ## Parameters
        /// options:
        /// See `rerun::SpawnOptions` for more information.
        ///
        /// flush_timeout_sec:
        /// The minimum time the SDK will wait during a flush before potentially
        /// dropping data if progress is not being made. Passing a negative value indicates no
        /// timeout, and can cause a call to `flush` to block indefinitely.
        Error spawn(const SpawnOptions& options = {}, float flush_timeout_sec = 2.0) const;

        /// @see RecordingStream::spawn
        template <typename TRep, typename TPeriod>
        Error spawn(
            const SpawnOptions& options = {},
            std::chrono::duration<TRep, TPeriod> flush_timeout = std::chrono::seconds(2)
        ) const {
            using seconds_float = std::chrono::duration<float>; // Default ratio is 1:1 == seconds.
            return spawn(options, std::chrono::duration_cast<seconds_float>(flush_timeout).count());
        }

        /// Stream all log-data to a given `.rrd` file.
        ///
        /// This function returns immediately.
        Error save(std::string_view path) const;

        /// Stream all log-data to standard output.
        ///
        /// Pipe the result into the Rerun Viewer to visualize it.
        ///
        /// If there isn't any listener at the other end of the pipe, the `RecordingStream` will
        /// default back to `buffered` mode, in order not to break the user's terminal.
        ///
        /// This function returns immediately.
        //
        // NOTE: This should be called `stdout` like in other SDK, but turns out that `stdout` is a
        // macro when compiling with msvc [1].
        // [1]: https://learn.microsoft.com/en-us/cpp/c-runtime-library/stdin-stdout-stderr?view=msvc-170
        Error to_stdout() const;

        /// Initiates a flush the batching pipeline and waits for it to propagate.
        ///
        /// See `RecordingStream` docs for ordering semantics and multithreading guarantees.
        void flush_blocking() const;

        /// @}

        // -----------------------------------------------------------------------------------------
        /// \name Controlling log time.
        /// \details
        /// @{

        /// Set the current time of the recording, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_sequence("frame_nr", frame_nr)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_seconds, set_time_nanos, reset_time, set_time, disable_timeline
        void set_time_sequence(std::string_view timeline_name, int64_t sequence_nr) const;

        /// Set the current time of the recording, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time("sim_time", sim_time_secs)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_sequence, set_time_seconds, set_time_nanos, reset_time, disable_timeline
        template <typename TClock>
        void set_time(std::string_view timeline_name, std::chrono::time_point<TClock> time) const {
            set_time(timeline_name, time.time_since_epoch());
        }

        /// Set the current time of the recording, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time("sim_time", sim_time_secs)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_sequence, set_time_seconds, set_time_nanos, reset_time, disable_timeline
        template <typename TRep, typename TPeriod>
        void set_time(std::string_view timeline_name, std::chrono::duration<TRep, TPeriod> time)
            const {
            if constexpr (std::is_floating_point<TRep>::value) {
                using seconds_double =
                    std::chrono::duration<double>; // Default ratio is 1:1 == seconds.
                set_time_seconds(
                    timeline_name,
                    std::chrono::duration_cast<seconds_double>(time).count()
                );
            } else {
                set_time_nanos(
                    timeline_name,
                    std::chrono::duration_cast<std::chrono::nanoseconds>(time).count()
                );
            }
        }

        /// Set the current time of the recording, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_seconds("sim_time", sim_time_secs)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_sequence, set_time_nanos, reset_time, set_time, disable_timeline
        void set_time_seconds(std::string_view timeline_name, double seconds) const;

        /// Set the current time of the recording, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_nanos("sim_time", sim_time_nanos)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_sequence, set_time_seconds, reset_time, set_time, disable_timeline
        void set_time_nanos(std::string_view timeline_name, int64_t nanos) const;

        /// Stops logging to the specified timeline for subsequent log calls.
        ///
        /// The timeline is still there, but will not be updated with any new data.
        ///
        /// No-op if the timeline doesn't exist.
        ///
        /// @see set_time_sequence, set_time_seconds, set_time, reset_time
        void disable_timeline(std::string_view timeline_name) const;

        /// Clears out the current time of the recording, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.reset_time()`.
        /// @see set_time_sequence, set_time_seconds, set_time_nanos, disable_timeline
        void reset_time() const;

        /// @}

        // -----------------------------------------------------------------------------------------
        /// \name Logging
        /// @{

        /// Logs one or more archetype and/or component batches.
        ///
        /// This is the main entry point for logging data to rerun. It can be used to log anything
        /// that implements the `AsComponents<T>` trait.
        ///
        /// When logging data, you must always provide an [entity_path](https://www.rerun.io/docs/concepts/entity-path)
        /// for identifying the data. Note that the path prefix "rerun/" is considered reserved for use by the Rerun SDK
        /// itself and should not be used for logging user data. This is where Rerun will log additional information
        /// such as warnings.
        ///
        /// The most common way to log is with one of the rerun archetypes, all of which implement the `AsComponents` trait.
        ///
        /// For example, to log two 3D points:
        /// ```
        /// rec.log("my/point", rerun::Points3D({{0.0f, 0.0f, 0.0f}, {1.0f, 1.0f, 1.0f}}));
        /// ```
        ///
        /// The `log` function can flexibly accept an arbitrary number of additional objects which will
        /// be merged into the first entity so long as they don't expose conflicting components, for instance:
        /// ```
        /// // Log three points with arrows sticking out of them:
        /// rec.log(
        ///     "my/points",
        ///     rerun::Points3D({{0.2f, 0.5f, 0.3f}, {0.9f, 1.2f, 0.1f}, {1.0f, 4.2f, 0.3f}})
        ///             .with_radii({0.1, 0.2, 0.3}),
        ///     rerun::Arrows3D::from_vectors({{0.3f, 2.1f, 0.2f}, {0.9f, -1.1, 2.3f}, {-0.4f, 0.5f, 2.9f}})
        /// );
        /// ```
        ///
        /// Any failures that may occur during serialization are handled with `Error::handle`.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param archetypes_or_collectiones Any type for which the `AsComponents<T>` trait is implemented.
        /// This is the case for any archetype or `std::vector`/`std::array`/C-array of components implements.
        ///
        /// @see try_log, log_timeless, try_log_with_timeless
        template <typename... Ts>
        void log(std::string_view entity_path, const Ts&... archetypes_or_collectiones) const {
            if (!is_enabled()) {
                return;
            }
            try_log_with_timeless(entity_path, false, archetypes_or_collectiones...).handle();
        }

        /// Logs one or more archetype and/or component batches as timeless data.
        ///
        /// Like `log` but logs the data as timeless:
        /// Timeless data is present on all timelines and behaves as if it was recorded infinitely
        /// far into the past.
        ///
        /// Failures are handled with `Error::handle`.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param archetypes_or_collectiones Any type for which the `AsComponents<T>` trait is implemented.
        /// This is the case for any archetype or `std::vector`/`std::array`/C-array of components implements.
        ///
        /// @see log, try_log_timeless, try_log_with_timeless
        template <typename... Ts>
        void log_timeless(std::string_view entity_path, const Ts&... archetypes_or_collectiones)
            const {
            if (!is_enabled()) {
                return;
            }
            try_log_with_timeless(entity_path, true, archetypes_or_collectiones...).handle();
        }

        /// Logs one or more archetype and/or component batches.
        ///
        /// See `log` for more information.
        /// Unlike `log` this method returns an error if an error occurs during serialization or logging.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param archetypes_or_collectiones Any type for which the `AsComponents<T>` trait is implemented.
        /// This is the case for any archetype or `std::vector`/`std::array`/C-array of components implements.
        ///
        /// @see log, try_log_timeless, try_log_with_timeless
        template <typename... Ts>
        Error try_log(std::string_view entity_path, const Ts&... archetypes_or_collectiones) const {
            if (!is_enabled()) {
                return Error::ok();
            }
            return try_log_with_timeless(entity_path, false, archetypes_or_collectiones...);
        }

        /// Logs one or more archetype and/or component batches as timeless data, returning an error.
        ///
        /// See `log`/`log_timeless` for more information.
        /// Unlike `log_timeless` this method returns if an error occurs during serialization or logging.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param archetypes_or_collectiones Any type for which the `AsComponents<T>` trait is implemented.
        /// This is the case for any archetype or `std::vector`/`std::array`/C-array of components implements.
        /// \returns An error if an error occurs during serialization or logging.
        ///
        /// @see log_timeless, try_log, try_log_with_timeless
        template <typename... Ts>
        Error try_log_timeless(
            std::string_view entity_path, const Ts&... archetypes_or_collectiones
        ) const {
            if (!is_enabled()) {
                return Error::ok();
            }
            return try_log_with_timeless(entity_path, true, archetypes_or_collectiones...);
        }

        /// Logs one or more archetype and/or component batches optionally timeless, returning an error.
        ///
        /// See `log`/`log_timeless` for more information.
        /// Returns an error if an error occurs during serialization or logging.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param timeless If true, the logged components will be timeless.
        /// Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        /// Additional timelines set by `set_time_sequence` or `set_time` will also be included.
        /// \param archetypes_or_collectiones Any type for which the `AsComponents<T>` trait is implemented.
        /// This is the case for any archetype or `std::vector`/`std::array`/C-array of components implements.
        /// \returns An error if an error occurs during serialization or logging.
        ///
        /// @see log, try_log, log_timeless, try_log_timeless
        template <typename... Ts>
        Error try_log_with_timeless(
            std::string_view entity_path, bool timeless, const Ts&... archetypes_or_collectiones
        ) const {
            if (!is_enabled()) {
                return Error::ok();
            }
            std::vector<DataCell> serialized_batches;
            Error err;
            (
                [&] {
                    if (err.is_err()) {
                        return;
                    }

                    const Result<std::vector<DataCell>> serialization_result =
                        AsComponents<Ts>().serialize(archetypes_or_collectiones);
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

            return try_log_serialized_batches(entity_path, timeless, std::move(serialized_batches));
        }

        /// Logs several serialized batches batches, returning an error on failure.
        ///
        /// This is a more low-level API than `log`/`log_timeless\ and requires you to already serialize the data
        /// ahead of time.
        ///
        /// The number of instances in each batch must either be equal to the maximum or:
        /// - zero instances - implies a clear
        /// - single instance (but other instances have more) - causes a splat
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param timeless If true, the logged components will be timeless.
        /// Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        /// Additional timelines set by `set_time_sequence` or `set_time` will also be included.
        /// \param batches The serialized batches to log.
        ///
        /// \see `log`, `try_log`, `log_timeless`, `try_log_timeless`, `try_log_with_timeless`
        Error try_log_serialized_batches(
            std::string_view entity_path, bool timeless, std::vector<DataCell> batches
        ) const;

        /// Bottom level API that logs raw data cells to the recording stream.
        ///
        /// In order to use this you need to pass serialized Arrow data cells.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param num_instances
        /// Each cell is expected to hold exactly `num_instances` instances.
        /// \param num_data_cells Number of data cells passed in.
        /// \param data_cells The data cells to log.
        /// \param inject_time
        /// If set to `true`, the row's timestamp data will be overridden using the recording
        /// streams internal clock.
        ///
        /// \see `try_log_serialized_batches`
        Error try_log_data_row(
            std::string_view entity_path, size_t num_instances, size_t num_data_cells,
            const DataCell* data_cells, bool inject_time
        ) const;

        /// @}

      private:
        RecordingStream(uint32_t id, StoreKind store_kind);

        uint32_t _id;
        StoreKind _store_kind;
        bool _enabled;
    };
} // namespace rerun
