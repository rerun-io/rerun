use re_memory::TrackingStatistics;

pub fn install_memory_use_meters() {
    let meter = opentelemetry::global::meter("memory-use");

    meter
        .i64_observable_gauge("memory_resident_set_size_bytes")
        .with_description("Resident Set Size")
        .with_unit("B")
        .with_callback(|observer| {
            let bytes_used = memory_stats::memory_stats().map(|usage| usage.physical_mem as i64);
            if let Some(bytes_used) = bytes_used {
                observer.observe(bytes_used, &[]);
            }
        })
        .build();

    tokio::spawn(memory_monitor_task());
}

/// Monitors memory use periodically,
/// and logs memory stats each time we cross another GiB of allocated memory.
async fn memory_monitor_task() {
    // TODO: set SMALL_SIZE/MEDIUM_SIZE things.
    re_memory::accounting_allocator::set_tracking_callstacks(true);

    const ONE_GIG: u64 = 1024 * 1024 * 1024;

    // How often we check memory use
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

    let total_ram_in_bytes = re_memory::total_ram_in_bytes();

    let mut warn_at_gb = if total_ram_in_bytes == 0 {
        tracing::warn!("Failed to estimate how much RAM is in this machine");
        4 // First warning when we cross this many GiB
    } else {
        let total_ram_gb = total_ram_in_bytes / ONE_GIG;
        total_ram_gb / 2 // First warning when we cross 50%
    };
    tracing::info!("Will log memory stats when we first pass {warn_at_gb} GiB");

    loop {
        interval.tick().await;

        let current_ram = re_memory::MemoryUse::capture();

        let used_bytes = current_ram.resident.or(current_ram.counted);

        let Some(used_bytes) = used_bytes else {
            tracing::warn!("Failed to query current RAM use");
            return;
        };
        let used_bytes = used_bytes as u64;

        let used_gb_floored = used_bytes / ONE_GIG;

        // Check if we've crossed into a new GB threshold:
        if warn_at_gb <= used_gb_floored {
            warn_at_gb = used_gb_floored + 1;

            if total_ram_in_bytes == 0 {
                tracing::info!(
                    "Using {:.1} / {:.1} GiB RAM",
                    used_bytes as f64 / ONE_GIG as f64,
                    total_ram_in_bytes as f64 / ONE_GIG as f64
                );
            } else {
                tracing::info!(
                    "Using {:.1} / ? GiB RAM",
                    used_bytes as f64 / ONE_GIG as f64
                );
            }

            if let Some(stats) = re_memory::accounting_allocator::tracking_stats() {
                let re_memory::TrackingStatistics {
                    track_size_threshold: _,
                    untracked,
                    stochastically_tracked,
                    fully_tracked,
                    overhead,
                    top_callstacks,
                } = stats;
                tracing::info!(
                    overhead_bytes = overhead.size,
                    untracked_bytes = untracked.size,
                    stochastically_tracked_bytes = stochastically_tracked.size,
                    fully_tracked_bytes = fully_tracked.size,
                    "Detailed allocator stats"
                );

                for (i, callstack) in top_callstacks.iter().enumerate().take(10) {
                    let re_memory::CallstackStatistics {
                        readable_backtrace,
                        stochastic_rate,
                        extant,
                    } = callstack;
                    tracing::debug!(
                        extant_count = stochastic_rate * extant.count,
                        extant_bytes = stochastic_rate * extant.size,
                        stacktrace = readable_backtrace.as_str(),
                        "Highest allocator #{i}"
                    );
                }
            } else {
                re_log::warn_once!("re_memory accounting allocator not installed");
            }
        }
    }
}
