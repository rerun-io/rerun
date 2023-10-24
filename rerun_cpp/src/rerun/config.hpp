#pragma once
#include <atomic>

#ifndef RERUN_ENABLED
#define RERUN_ENABLED 1
#endif

namespace rerun {
    struct RerunGlobalConfig {
        static RerunGlobalConfig& instance();

        RerunGlobalConfig(const RerunGlobalConfig&) = delete;
        RerunGlobalConfig& operator=(const RerunGlobalConfig&) = delete;

        std::atomic_bool enabled;

      private:
        RerunGlobalConfig();

        ~RerunGlobalConfig() {}
    };

    /// Enable/disable all Rerun log statements.
    ///
    /// The default value of enabled is controlled by the RERUN environment variable.
    ///
    /// If RERUN is set to 1, true, or yes, then Rerun is enabled.
    /// If RERUN is set to 0, false, or no, then Rerun is disabled.
    ///
    /// RERUN can also be compile-timed disabled by compiling with `-DRERUN_ENABLED=0`
    inline void set_enabled(bool enabled) {
#if RERUN_ENABLED
        RerunGlobalConfig::instance().enabled.store(enabled, std::memory_order_seq_cst);
#else
        fprintf(
            stderr,
            "Tried to call set_enabled but rerun was compiled with RERUN_ENABLED=0",
            env
        );
#endif
    }

    /// Check if Rerun is enabled.
    inline bool is_enabled() {
        return RerunGlobalConfig::instance().enabled.load(std::memory_order_relaxed);
    }
} // namespace rerun
