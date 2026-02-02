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
}
