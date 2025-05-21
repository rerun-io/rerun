#pragma once

#include <chrono>
#include <cstdint> // uint32_t etc.
#include <filesystem>
#include <optional>
#include <string_view>
#include <type_traits>
#include <vector>

#include "as_components.hpp"
#include "component_column.hpp"
#include "error.hpp"
#include "spawn_options.hpp"
#include "time_column.hpp"

namespace rerun {
    struct ComponentBatch;

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
    /// See [SDK Micro Batching](https://www.rerun.io/docs/reference/sdk/micro-batching) for
    /// more information.
    ///
    /// The data will be timestamped automatically based on the `RecordingStream`'s
    /// internal clock.
    class RecordingStream {
      private:
        // TODO(grtlr): Ideally we'd expose more of the `EntityPath` struct to the C++ world so
        //              that we don't have to hardcode this here.
        static constexpr const char PROPERTIES_ENTITY_PATH[] = "__properties/";
        static constexpr const char RECORDING_PROPERTIES_ENTITY_PATH[] = "__properties/recording/";

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

        /// Connect to a remote Rerun Viewer on the given URL.
        ///
        /// Requires that you first start a Rerun Viewer by typing 'rerun' in a terminal.
        ///
        /// url:
        /// The scheme must be one of `rerun://`, `rerun+http://`, or `rerun+https://`,
        /// and the pathname must be `/proxy`.
        ///
        /// The default is `rerun+http://127.0.0.1:9876/proxy`.
        ///
        /// flush_timeout_sec:
        /// The minimum time the SDK will wait during a flush before potentially
        /// dropping data if progress is not being made. Passing a negative value indicates no
        /// timeout, and can cause a call to `flush` to block indefinitely.
        ///
        /// This function returns immediately.
        Error connect_grpc(
            std::string_view url = "rerun+http://127.0.0.1:9876/proxy",
            float flush_timeout_sec = 2.0
        ) const;

        /// Swaps the underlying sink for a gRPC server sink pre-configured to listen on `rerun+http://{bind_ip}:{port}/proxy`.
        ///
        /// The gRPC server will buffer all log data in memory so that late connecting viewers will get all the data.
        /// You can limit the amount of data buffered by the gRPC server with the `server_memory_limit` argument.
        /// Once reached, the earliest logged data will be dropped. Static data is never dropped.
        ///
        /// Returns the URI of the gRPC server so you can connect to it from a viewer.
        ///
        /// This function returns immediately.
        Result<std::string> serve_grpc(
            std::string_view bind_ip = "0.0.0.0", uint16_t port = 9876,
            std::string_view server_memory_limit = "75%"
        ) const;

        /// Spawns a new Rerun Viewer process from an executable available in PATH, then connects to it
        /// over gRPC.
        ///
        /// flush_timeout_sec:
        /// The minimum time the SDK will wait during a flush before potentially
        /// dropping data if progress is not being made. Passing a negative value indicates no
        /// timeout, and can cause a call to `flush` to block indefinitely.
        ///
        /// If a Rerun Viewer is already listening on this port, the stream will be redirected to
        /// that viewer instead of starting a new one.
        ///
        /// ## Parameters
        /// options:
        /// See `rerun::SpawnOptions` for more information.
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
        /// The Rerun Viewer is able to read continuously from the resulting rrd file while it is being written.
        /// However, depending on your OS and configuration, changes may not be immediately visible due to file caching.
        /// This is a common issue on Windows and (to a lesser extent) on MacOS.
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
        /// \name Controlling log time (index).
        /// \details
        /// @{

        /// Set the index value of the given timeline as a sequence number, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_sequence("frame_nr", frame_nr)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_sequence, set_time_duration, set_time_duration_secs, set_time_duration_nanos, set_time_timestamp, set_time_timestamp_secs_since_epoch, set_time_timestamp_nanos_since_epoch
        void set_time_sequence(std::string_view timeline_name, int64_t sequence_nr) const;

        /// Set the index value of the given timeline as a duration, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_duration("runtime", time_since_start)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_sequence, set_time_duration, set_time_duration_secs, set_time_duration_nanos, set_time_timestamp, set_time_timestamp_secs_since_epoch, set_time_timestamp_nanos_since_epoch
        template <typename TRep, typename TPeriod>
        void set_time_duration(
            std::string_view timeline_name, std::chrono::duration<TRep, TPeriod> duration
        ) const {
            auto nanos = std::chrono::duration_cast<std::chrono::nanoseconds>(duration).count();
            set_time_duration_nanos(timeline_name, nanos);
        }

        /// Set the index value of the given timeline as a duration in seconds, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_duration_secs("runtime", seconds_since_start)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_sequence, set_time_duration, set_time_duration_secs, set_time_duration_nanos, set_time_timestamp, set_time_timestamp_secs_since_epoch, set_time_timestamp_nanos_since_epoch
        void set_time_duration_secs(std::string_view timeline_name, double secs) const {
            set_time_duration_nanos(timeline_name, static_cast<int64_t>(1e9 * secs + 0.5));
        }

        /// Set the index value of the given timeline as a duration in nanoseconds, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_duration_nanos("runtime", nanos_since_start)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_sequence, set_time_duration, set_time_duration_secs, set_time_duration_nanos, set_time_timestamp, set_time_timestamp_secs_since_epoch, set_time_timestamp_nanos_since_epoch
        void set_time_duration_nanos(std::string_view timeline_name, int64_t nanos) const;

        /// Set the index value of the given timeline as a timestamp, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_timestamp("capture_time", now())`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_sequence, set_time_duration, set_time_duration_secs, set_time_duration_nanos, set_time_timestamp, set_time_timestamp_secs_since_epoch, set_time_timestamp_nanos_since_epoch
        template <typename TClock>
        void set_time_timestamp(
            std::string_view timeline_name, std::chrono::time_point<TClock> timestamp
        ) const {
            set_time_timestamp_nanos_since_epoch(
                timeline_name,
                std::chrono::duration_cast<std::chrono::nanoseconds>(timestamp.time_since_epoch())
                    .count()
            );
        }

        /// Set the index value of the given timeline as seconds since Unix Epoch (1970), for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_timestamp_secs_since_epoch("capture_time", secs_since_epoch())`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_sequence, set_time_duration, set_time_duration_secs, set_time_duration_nanos, set_time_timestamp, set_time_timestamp_secs_since_epoch, set_time_timestamp_nanos_since_epoch
        void set_time_timestamp_secs_since_epoch(std::string_view timeline_name, double seconds)
            const {
            set_time_timestamp_nanos_since_epoch(
                timeline_name,
                static_cast<int64_t>(1e9 * seconds)
            );
        }

        /// Set the index value of the given timeline as nanoseconds since Unix Epoch (1970), for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_timestamp_nanos_since_epoch("capture_time", nanos_since_epoch())`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_sequence, set_time_duration, set_time_duration_secs, set_time_duration_nanos, set_time_timestamp, set_time_timestamp_secs_since_epoch, set_time_timestamp_nanos_since_epoch
        void set_time_timestamp_nanos_since_epoch(std::string_view timeline_name, int64_t nanos)
            const;

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
        [[deprecated("Renamed to `set_time_timestamp`")]] void set_time(
            std::string_view timeline_name, std::chrono::time_point<TClock> time
        ) const {
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
        [[deprecated("Renamed `set_time_duration`")]] void set_time(
            std::string_view timeline_name, std::chrono::duration<TRep, TPeriod> time
        ) const {
            set_time_duration(timeline_name, time);
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
        [[deprecated("Use either `set_time_duration_secs` or `set_time_timestamp_secs_since_epoch`"
        )]] void
            set_time_seconds(std::string_view timeline_name, double seconds) const {
            set_time_duration_secs(timeline_name, seconds);
        }

        /// Set the current time of the recording, for the current calling thread.
        ///
        /// Used for all subsequent logging performed from this same thread, until the next call
        /// to one of the time setting methods.
        ///
        /// For example: `rec.set_time_nanos("sim_time", sim_time_nanos)`.
        ///
        /// You can remove a timeline from subsequent log calls again using `rec.disable_timeline`.
        /// @see set_time_sequence, set_time_seconds, reset_time, set_time, disable_timeline
        [[deprecated(
            "Use either `set_time_duration_nanos` or `set_time_timestamp_nanos_since_epoch`"
        )]] void
            set_time_nanos(std::string_view timeline_name, int64_t nanos) const {
            set_time_duration_nanos(timeline_name, nanos);
        }

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
        /// \name Sending & logging data.
        /// @{

        /// Logs one or more archetype and/or component batches.
        ///
        /// This is the main entry point for logging data to rerun. It can be used to log anything
        /// that implements the `AsComponents<T>` trait.
        ///
        /// When logging data, you must always provide an [entity_path](https://www.rerun.io/docs/concepts/entity-path)
        /// for identifying the data. Note that paths prefixed with "__" are considered reserved for use by the Rerun SDK
        /// itself and should not be used for logging user data. This is where Rerun will log additional information
        /// such as properties and warnings.
        ///
        /// The most common way to log is with one of the rerun archetypes, all of which implement the `AsComponents` trait.
        ///
        /// For example, to log two 3D points:
        /// ```
        /// rec.log("my/point", rerun::Points3D({{0.0f, 0.0f, 0.0f}, {1.0f, 1.0f, 1.0f}}));
        /// ```
        ///
        /// The `log` function can flexibly accept an arbitrary number of additional objects which will
        /// be merged into the first entity, for instance:
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
        /// Any failures that may are handled with `Error::handle`.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param as_components Any type for which the `AsComponents<T>` trait is implemented.
        /// This is the case for any archetype as well as individual or collection of `ComponentBatch`.
        /// You can implement `AsComponents` for your own types as well
        ///
        /// @see try_log, log_static, try_log_with_static
        template <typename... Ts>
        void log(std::string_view entity_path, const Ts&... as_components) const {
            if (!is_enabled()) {
                return;
            }
            try_log_with_static(entity_path, false, as_components...).handle();
        }

        /// Logs one or more archetype and/or component batches as static data.
        ///
        /// Like `log` but logs the data as static:
        /// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        /// any temporal data of the same type.
        ///
        /// Failures are handled with `Error::handle`.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param as_components Any type for which the `AsComponents<T>` trait is implemented.
        /// This is the case for any archetype as well as individual or collection of `ComponentBatch`.
        /// You can implement `AsComponents` for your own types as well
        ///
        /// @see log, try_log_static, try_log_with_static
        template <typename... Ts>
        void log_static(std::string_view entity_path, const Ts&... as_components) const {
            if (!is_enabled()) {
                return;
            }
            try_log_with_static(entity_path, true, as_components...).handle();
        }

        /// Logs one or more archetype and/or component batches.
        ///
        /// See `log` for more information.
        /// Unlike `log` this method returns an error if an error occurs.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param as_components Any type for which the `AsComponents<T>` trait is implemented.
        /// This is the case for any archetype as well as individual or collection of `ComponentBatch`.
        /// You can implement `AsComponents` for your own types as well
        ///
        /// @see log, try_log_static, try_log_with_static
        template <typename... Ts>
        Error try_log(std::string_view entity_path, const Ts&... as_components) const {
            if (!is_enabled()) {
                return Error::ok();
            }
            return try_log_with_static(entity_path, false, as_components...);
        }

        /// Logs one or more archetype and/or component batches as static data, returning an error.
        ///
        /// See `log`/`log_static` for more information.
        /// Unlike `log_static` this method returns if an error occurs.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param as_components Any type for which the `AsComponents<T>` trait is implemented.
        /// This is the case for any archetype as well as individual or collection of `ComponentBatch`.
        /// You can implement `AsComponents` for your own types as well
        /// \returns An error if an error occurs during evaluation of `AsComponents` or logging.
        ///
        /// @see log_static, try_log, try_log_with_static
        template <typename... Ts>
        Error try_log_static(std::string_view entity_path, const Ts&... as_components) const {
            if (!is_enabled()) {
                return Error::ok();
            }
            return try_log_with_static(entity_path, true, as_components...);
        }

        /// Logs one or more archetype and/or component batches optionally static, returning an error.
        ///
        /// See `log`/`log_static` for more information.
        /// Returns an error if an error occurs during evaluation of `AsComponents` or logging.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param static_ If true, the logged components will be static.
        /// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        /// any temporal data of the same type.
        /// Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        /// Additional timelines set by `set_time_sequence` or `set_time` will also be included.
        /// \param as_components Any type for which the `AsComponents<T>` trait is implemented.
        /// This is the case for any archetype as well as individual or collection of `ComponentBatch`.
        /// You can implement `AsComponents` for your own types as well
        ///
        /// @see log, try_log, log_static, try_log_static
        template <typename... Ts>
        void log_with_static(std::string_view entity_path, bool static_, const Ts&... as_components)
            const {
            try_log_with_static(entity_path, static_, as_components...).handle();
        }

        /// Logs one or more archetype and/or component batches optionally static, returning an error.
        ///
        /// See `log`/`log_static` for more information.
        /// Returns an error if an error occurs during evaluation of `AsComponents` or logging.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param static_ If true, the logged components will be static.
        /// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        /// any temporal data of the same type.
        /// Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        /// Additional timelines set by `set_time_sequence` or `set_time` will also be included.
        /// \param as_components Any type for which the `AsComponents<T>` trait is implemented.
        /// This is the case for any archetype as well as individual or collection of `ComponentBatch`.
        /// You can implement `AsComponents` for your own types as well
        /// \returns An error if an error occurs during evaluation of `AsComponents` or logging.
        ///
        /// @see log, try_log, log_static, try_log_static
        template <typename... Ts>
        Error try_log_with_static(
            std::string_view entity_path, bool static_, const Ts&... as_components
        ) const {
            if (!is_enabled()) {
                return Error::ok();
            }
            std::vector<ComponentBatch> serialized_columns;
            Error err;
            (
                [&] {
                    if (err.is_err()) {
                        return;
                    }

                    const Result<Collection<ComponentBatch>> serialization_result =
                        AsComponents<Ts>().as_batches(as_components);
                    if (serialization_result.is_err()) {
                        err = serialization_result.error;
                        return;
                    }

                    if (serialized_columns.empty()) {
                        // Fast path for the first batch (which is usually the only one!)
                        serialized_columns = std::move(serialization_result.value).to_vector();
                    } else {
                        serialized_columns.insert(
                            serialized_columns.end(),
                            std::make_move_iterator(serialization_result.value.begin()),
                            std::make_move_iterator(serialization_result.value.end())
                        );
                    }
                }(),
                ...
            );
            RR_RETURN_NOT_OK(err);

            return try_log_serialized_batches(entity_path, static_, std::move(serialized_columns));
        }

        /// Logs several serialized batches batches, returning an error on failure.
        ///
        /// This is a more low-level API than `log`/`log_static\ and requires you to already serialize the data
        /// ahead of time.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param static_ If true, the logged components will be static.
        /// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        /// any temporal data of the same type.
        /// Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        /// Additional timelines set by `set_time_sequence` or `set_time` will also be included.
        /// \param batches The serialized batches to log.
        ///
        /// \see `log`, `try_log`, `log_static`, `try_log_static`, `try_log_with_static`
        Error try_log_serialized_batches(
            std::string_view entity_path, bool static_, std::vector<ComponentBatch> batches
        ) const;

        /// Bottom level API that logs raw data cells to the recording stream.
        ///
        /// In order to use this you need to pass serialized Arrow data cells.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param num_data_cells Number of data cells passed in.
        /// \param data_cells The data cells to log.
        /// \param inject_time
        /// If set to `true`, the row's timestamp data will be overridden using the recording
        /// streams internal clock.
        ///
        /// \see `try_log_serialized_batches`
        Error try_log_data_row(
            std::string_view entity_path, size_t num_data_cells, const ComponentBatch* data_cells,
            bool inject_time
        ) const;

        /// Logs the file at the given `path` using all `DataLoader`s available.
        ///
        /// A single `path` might be handled by more than one loader.
        ///
        /// This method blocks until either at least one `DataLoader` starts streaming data in
        /// or all of them fail.
        ///
        /// See <https://www.rerun.io/docs/reference/data-loaders/overview> for more information.
        ///
        /// \param filepath Path to the file to be logged.
        /// \param entity_path_prefix What should the logged entity paths be prefixed with?
        /// \param static_ If true, the logged components will be static.
        /// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        /// any temporal data of the same type.
        /// Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        /// Additional timelines set by `set_time_sequence` or `set_time` will also be included.
        ///
        /// \see `try_log_file_from_path`
        void log_file_from_path(
            const std::filesystem::path& filepath,
            std::string_view entity_path_prefix = std::string_view(), bool static_ = false
        ) const {
            try_log_file_from_path(filepath, entity_path_prefix, static_).handle();
        }

        /// Logs the file at the given `path` using all `DataLoader`s available.
        ///
        /// A single `path` might be handled by more than one loader.
        ///
        /// This method blocks until either at least one `DataLoader` starts streaming data in
        /// or all of them fail.
        ///
        /// See <https://www.rerun.io/docs/reference/data-loaders/overview> for more information.
        ///
        /// \param filepath Path to the file to be logged.
        /// \param entity_path_prefix What should the logged entity paths be prefixed with?
        /// \param static_ If true, the logged components will be static.
        /// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        /// any temporal data of the same type.
        /// Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        /// Additional timelines set by `set_time_sequence` or `set_time` will also be included.
        ///
        /// \see `log_file_from_path`
        Error try_log_file_from_path(
            const std::filesystem::path& filepath,
            std::string_view entity_path_prefix = std::string_view(), bool static_ = false
        ) const;

        /// Logs the given `contents` using all `DataLoader`s available.
        ///
        /// A single `path` might be handled by more than one loader.
        ///
        /// This method blocks until either at least one `DataLoader` starts streaming data in
        /// or all of them fail.
        ///
        /// See <https://www.rerun.io/docs/reference/data-loaders/overview> for more information.
        ///
        /// \param filepath Path to the file that the `contents` belong to.
        /// \param contents Contents to be logged.
        /// \param contents_size Size in bytes of the `contents`.
        /// \param entity_path_prefix What should the logged entity paths be prefixed with?
        /// \param static_ If true, the logged components will be static.
        /// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        /// any temporal data of the same type.
        /// Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        /// Additional timelines set by `set_time_sequence` or `set_time` will also be included.
        ///
        /// \see `try_log_file_from_contents`
        void log_file_from_contents(
            const std::filesystem::path& filepath, const std::byte* contents, size_t contents_size,
            std::string_view entity_path_prefix = std::string_view(), bool static_ = false
        ) const {
            try_log_file_from_contents(
                filepath,
                contents,
                contents_size,
                entity_path_prefix,
                static_
            )
                .handle();
        }

        /// Logs the given `contents` using all `DataLoader`s available.
        ///
        /// A single `path` might be handled by more than one loader.
        ///
        /// This method blocks until either at least one `DataLoader` starts streaming data in
        /// or all of them fail.
        ///
        /// See <https://www.rerun.io/docs/reference/data-loaders/overview> for more information.
        ///
        /// \param filepath Path to the file that the `contents` belong to.
        /// \param contents Contents to be logged.
        /// \param contents_size Size in bytes of the `contents`.
        /// \param entity_path_prefix What should the logged entity paths be prefixed with?
        /// \param static_ If true, the logged components will be static.
        /// Static data has no time associated with it, exists on all timelines, and unconditionally shadows
        /// any temporal data of the same type.
        /// Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
        /// Additional timelines set by `set_time_sequence` or `set_time` will also be included.
        ///
        /// \see `log_file_from_contents`
        Error try_log_file_from_contents(
            const std::filesystem::path& filepath, const std::byte* contents, size_t contents_size,
            std::string_view entity_path_prefix = std::string_view(), bool static_ = false
        ) const;

        /// Directly log a columns of data to Rerun.
        ///
        /// This variant takes in arbitrary amount of `ComponentColumn`s and `ComponentColumn` collections.
        ///
        /// Unlike the regular `log` API, which is row-oriented, this API lets you submit the data
        /// in a columnar form. Each `TimeColumn` and `ComponentColumn` represents a column of data that will be sent to Rerun.
        /// The lengths of all of these columns must match, and all
        /// data that shares the same index across the different columns will act as a single logical row,
        /// equivalent to a single call to `RecordingStream::log`.
        ///
        /// Note that this API ignores any stateful time set on the log stream via the `RecordingStream::set_time_*` APIs.
        /// Furthermore, this will _not_ inject the default timelines `log_tick` and `log_time` timeline columns.
        ///
        /// Any failures that may occur during serialization are handled with `Error::handle`.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param time_columns The time columns to send.
        /// \param component_columns The columns of components to send. Both individual `ComponentColumn`s and `Collection<ComponentColumn>`s are accepted.
        /// \see `try_send_columns`
        template <typename... Ts>
        void send_columns(
            std::string_view entity_path, Collection<TimeColumn> time_columns,
            Ts... component_columns // NOLINT
        ) const {
            try_send_columns(entity_path, time_columns, component_columns...).handle();
        }

        /// Directly log a columns of data to Rerun.
        ///
        /// This variant takes in arbitrary amount of `ComponentColumn`s and `ComponentColumn` collections.
        ///
        /// Unlike the regular `log` API, which is row-oriented, this API lets you submit the data
        /// in a columnar form. Each `TimeColumn` and `ComponentColumn` represents a column of data that will be sent to Rerun.
        /// The lengths of all of these columns must match, and all
        /// data that shares the same index across the different columns will act as a single logical row,
        /// equivalent to a single call to `RecordingStream::log`.
        ///
        /// Note that this API ignores any stateful time set on the log stream via the `RecordingStream::set_time_*` APIs.
        /// Furthermore, this will _not_ inject the default timelines `log_tick` and `log_time` timeline columns.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param time_columns The time columns to send.
        /// \param component_columns The columns of components to send. Both individual `ComponentColumn`s and `Collection<ComponentColumn>`s are accepted.
        /// \see `send_columns`
        template <typename... Ts>
        Error try_send_columns(
            std::string_view entity_path, Collection<TimeColumn> time_columns,
            Ts... component_columns // NOLINT
        ) const {
            if constexpr (sizeof...(Ts) == 1) {
                // Directly forward if this is only a single element,
                // skipping collection of component column vector.
                return try_send_columns(
                    entity_path,
                    std::move(time_columns),
                    Collection(std::forward<Ts...>(component_columns...))
                );
            }

            std::vector<ComponentColumn> flat_column_list;
            (
                [&] {
                    static_assert(
                        std::is_same_v<std::remove_cv_t<Ts>, ComponentColumn> ||
                            std::is_constructible_v<Collection<ComponentColumn>, Ts>,
                        "Ts must be ComponentColumn or a collection thereof"
                    );

                    push_back_columns(flat_column_list, std::move(component_columns));
                }(),
                ...
            );
            return try_send_columns(
                entity_path,
                std::move(time_columns),
                // Need to create collection explicitly, otherwise this becomes a recursive call.
                Collection<ComponentColumn>(std::move(flat_column_list))
            );
        }

        /// Directly log a columns of data to Rerun.
        ///
        /// Unlike the regular `log` API, which is row-oriented, this API lets you submit the data
        /// in a columnar form. Each `TimeColumn` and `ComponentColumn` represents a column of data that will be sent to Rerun.
        /// The lengths of all of these columns must match, and all
        /// data that shares the same index across the different columns will act as a single logical row,
        /// equivalent to a single call to `RecordingStream::log`.
        ///
        /// Note that this API ignores any stateful time set on the log stream via the `RecordingStream::set_time_*` APIs.
        /// Furthermore, this will _not_ inject the default timelines `log_tick` and `log_time` timeline columns.
        ///
        /// Any failures that may occur during serialization are handled with `Error::handle`.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param time_columns The time columns to send.
        /// \param component_columns The columns of components to send.
        /// \see `try_send_columns`
        void send_columns(
            std::string_view entity_path, Collection<TimeColumn> time_columns,
            Collection<ComponentColumn> component_columns
        ) const {
            try_send_columns(entity_path, time_columns, component_columns).handle();
        }

        /// Directly log a columns of data to Rerun.
        ///
        /// Unlike the regular `log` API, which is row-oriented, this API lets you submit the data
        /// in a columnar form. Each `TimeColumn` and `ComponentColumn` represents a column of data that will be sent to Rerun.
        /// The lengths of all of these columns must match, and all
        /// data that shares the same index across the different columns will act as a single logical row,
        /// equivalent to a single call to `RecordingStream::log`.
        ///
        /// Note that this API ignores any stateful time set on the log stream via the `RecordingStream::set_time_*` APIs.
        /// Furthermore, this will _not_ inject the default timelines `log_tick` and `log_time` timeline columns.
        ///
        /// \param entity_path Path to the entity in the space hierarchy.
        /// \param time_columns The time columns to send.
        /// \param component_columns The columns of components to send.
        /// \see `send_columns`
        Error try_send_columns(
            std::string_view entity_path, Collection<TimeColumn> time_columns,
            Collection<ComponentColumn> component_columns
        ) const;

        /// Set a property of a recording.
        ///
        /// Any failures that may occur during serialization are handled with `Error::handle`.
        ///
        /// \param name The name of the property.
        /// \param values The values of the property.
        /// \see `try_send_property`
        template <typename... Ts>
        void send_property(std::string_view name, const Ts&... values) const {
            try_send_property(name, values...).handle();
        }

        /// Set a property of a recording.
        ///
        /// Any failures that may occur during serialization are handled with `Error::handle`.
        ///
        /// \param name The name of the property.
        /// \param values The values of the property.
        /// \see `set_property`
        template <typename... Ts>
        Error try_send_property(std::string_view name, const Ts&... values) const {
            return try_log_static(
                this->PROPERTIES_ENTITY_PATH + std::string(name),
                values... // NOLINT
            );
        }

        /// Set the name of a recording.
        ///
        /// Any failures that may occur during serialization are handled with `Error::handle`.
        ///
        /// \param name The name of the recording.
        /// \see `try_send_recording_name`
        void send_recording_name(std::string_view name) const {
            try_send_recording_name(name).handle();
        }

        /// Set the name of a recording.
        ///
        /// \param name The name of the recording.
        /// \see `send_recording_name`
        Error try_send_recording_name(std::string_view name) const;

        /// Set the start time of a recording.
        ///
        /// Any failures that may occur during serialization are handled with `Error::handle`.
        ///
        /// \param nanos The timestamp of the recording in nanoseconds since Unix epoch.
        /// \see `try_send_recording_start_time`
        void send_recording_start_time_nanos(int64_t nanos) const {
            try_send_recording_start_time_nanos(nanos).handle();
        }

        /// Set the start time of a recording.
        ///
        /// \param nanos The timestamp of the recording in nanoseconds since Unix epoch.
        /// \see `set_name`
        Error try_send_recording_start_time_nanos(int64_t nanos) const;

        /// @}

      private:
        // Utility function to implement `try_send_columns` variadic template.
        static void push_back_columns(
            std::vector<ComponentColumn>& component_columns, Collection<ComponentColumn> new_columns
        ) {
            for (const auto& new_column : new_columns) {
                component_columns.emplace_back(std::move(new_column));
            }
        }

        static void push_back_columns(
            std::vector<ComponentColumn>& component_columns, ComponentColumn new_column
        ) {
            component_columns.emplace_back(std::move(new_column));
        }

        RecordingStream(uint32_t id, StoreKind store_kind);

        uint32_t _id;
        StoreKind _store_kind;
        bool _enabled;
    };
} // namespace rerun
