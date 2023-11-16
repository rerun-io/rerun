#pragma once
#include <atomic>

#ifndef RERUN_ENABLED
#define RERUN_ENABLED 1
#endif

namespace rerun {
    /// Configuration singleton that applies to the entire SDK.
    struct RerunGlobalConfig {
        static RerunGlobalConfig& instance();

        RerunGlobalConfig(const RerunGlobalConfig&) = delete;
        RerunGlobalConfig& operator=(const RerunGlobalConfig&) = delete;

        /// Whether `RecordingStream`s are enabled by default.
        ///
        /// \see set_default_enabled, is_default_enabled
        std::atomic_bool default_enabled;

      private:
        RerunGlobalConfig();

        ~RerunGlobalConfig() {}
    };

    /// Change whether `RecordingStream`s are enabled by default.
    ///
    /// This governs the creation of new `RecordingStream`s. If `default_enabled` is
    /// `false`, `RecordingStreams` will be created in the disabled state. Changing
    /// the value of `default_enabled` will not affect existing `RecordingStream`s.
    ///
    /// Note that regardless of usage of this API, the value of default_enabled will
    /// be overridden by the RERUN environment variable.
    ///
    /// If RERUN is set to `1`, `true`, or `yes`, then Rerun is enabled. If RERUN is
    /// set to `0`, `false`, or `no`, then Rerun is disabled.
    inline void set_default_enabled(bool default_enabled) {
        RerunGlobalConfig::instance().default_enabled.store(
            default_enabled,
            std::memory_order_seq_cst
        );
    }

    /// Check if Rerun is enabled.
    inline bool is_default_enabled() {
        // We use `memory_order_seq_cst` since this is only ever called during construction of
        // RecordingStreams. Consider changing to `memory_order_relaxed` if we need to call this
        // in a more frequently used code-path.
        return RerunGlobalConfig::instance().default_enabled.load(std::memory_order_seq_cst);
    }
} // namespace rerun
