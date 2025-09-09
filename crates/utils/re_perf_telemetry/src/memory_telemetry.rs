pub fn install_memory_use_meters() {
    let meter = opentelemetry::global::meter("memory-use");

    #[cfg(not(target_arch = "wasm32"))]
    {
        meter
            .i64_observable_gauge("RSS")
            .with_description("Resident Set Size")
            .with_unit("B")
            .with_callback(|observer| {
                let bytes_used =
                    memory_stats::memory_stats().map(|usage| usage.physical_mem as i64);
                if let Some(bytes_used) = bytes_used {
                    observer.observe(bytes_used, &[]);
                } else {
                    observer.observe(-666, &[]);
                }
            })
            .build();
    }
}
