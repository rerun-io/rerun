pub fn install_memory_use_meters() {
    let meter = opentelemetry::global::meter("memory-use");

    meter
        .i64_observable_gauge("memory_resident_set_size_bytes")
        .with_description("Resident Set Size")
        .with_unit("B")
        .with_callback(|observer| {
            #[expect(clippy::cast_possible_wrap)] // usize -> i64
            let bytes_used = memory_stats::memory_stats().map(|usage| usage.physical_mem as i64);
            if let Some(bytes_used) = bytes_used {
                observer.observe(bytes_used, &[]);
            }
        })
        .build();

    // ---------------------------------------------------

    if !re_memory::accounting_allocator::is_tracking_callstacks() {
        // NOTE: we don't turn it on here. It's up to the caller to opt-in to it, since it has a runtime cost.
        tracing::warn!(
            "Memory tracking turned OFF. Consider turning it on in your `fn main` with re_memory::accounting_allocator::set_tracking_callstacks(true);"
        );
    }

    // Periodically check memory usage:
    tokio::spawn(memory_monitor_task());
}

/// Monitors memory use periodically,
/// and logs memory stats each time we cross another GiB of allocated memory.
async fn memory_monitor_task() {
    const ONE_GIG: u64 = 1024 * 1024 * 1024;

    // How often we check RAM use.
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

    let total_ram_in_bytes = re_memory::total_ram_in_bytes();

    let mut warn_at_gb = if let Some(total_ram_in_bytes) = total_ram_in_bytes {
        let total_ram_gb = total_ram_in_bytes / ONE_GIG;
        total_ram_gb / 2 // First warning when we cross 50%
    } else {
        tracing::warn!("Failed to estimate how much RAM is in this machine");
        4 // First warning when we cross this many GiB
    };
    tracing::debug!("Will log memory stats when we first pass {warn_at_gb} GiB");

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

            if let Some(total_ram_in_bytes) = total_ram_in_bytes {
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
            }
        }
    }
}
