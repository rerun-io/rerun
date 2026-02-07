# re_perf_telemetry

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_perf_telemetry.svg)](https://crates.io/crates/re_perf_telemetry)
[![Documentation](https://docs.rs/re_perf_telemetry/badge.svg)](https://docs.rs/re_perf_telemetry)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

In and out of process telemetry and profiling utilities for Rerun & Redap.

Performance telemetry is always disabled by default. It is gated both by a feature flag (`perf_telemetry`) and runtime configuration in the form of environment variables:
* `TELEMETRY_ENABLED`: is performance telemetry enabled at all (default: `false`)?
* `TRACY_ENABLED`: is the tracy integration enabled (default: `false`)? works even if `TELEMETRY_ENABLED=false`, to reduce noise in measurements.
* `OTEL_SDK_ENABLED`: is the OpenTelemetry enabled (default: `false`)? does nothing if `TELEMETRY_ENABLED=false`.

Note that despite the name, this crate also hands all log output to the telemetry backend.

## What

`re_perf_telemetry` is a suite of developer tools that integrate with the `tracing` ecosystem and, by extension, make it possible to use:
* out-of-process IO-focused profilers such as the [OpenTelemetry](https://opentelemetry.io/) ecosystem (gRPC, trace propagation, distributed tracing, etc)
* in-process compute-focused profilers such as [Tracy](https://github.com/wolfpld/tracy)

What you can or cannot do with that depends on which project you're working on (Redap, Rerun SDK, Rerun Viewer). See below for more information.


### Redap

If you have source access to the Rerun Data Platform check the Readme there.


### Rerun SDK

In the Rerun SDK, `re_perf_telemetry` is always disabled by default (feature flagged), and only meant as a developer tool: it never ships with the final user builds.

The integration works pretty well for both out-of-process and in-process profiling.

We'll use the following script as an example:
```py
import rerun as rr

client = rr.catalog.CatalogClient("rerun://sandbox.redap.rerun.io")
client.dataset_entries()

dataset = client.get_dataset_entry(name="droid:raw")

# Get the RecordBatch reader from the query view
df = dataset.dataframe_query_view(
    index="real_time",
    contents={"/camera/ext1": ["VideoFrameReference:timestamp"]},
).df()

print(df.count())
```

* Example of out-of-process profiling using Jaeger (run `pixi run compose-dev` in the Redap repository to start a Jaeger instance):
  ```sh
  # Build the SDK with performance telemetry enabled, in the 'examples' environment:
  $ py-build-perf-examples

  # Run your script with both telemetry and the OpenTelemetry integration enabled:
  $ TELEMETRY_ENABLED=true OTEL_SDK_ENABLED=true <your_script>

  # Go to the Jaeger UI (http://localhost:16686/search) to look at the results
  ```
  <picture>
    <img src="https://static.rerun.io/re_perf_telemetry_sdk_jaeger/2a32ca041640e2902bf70164a42a1b539d7d759b/full.png" alt="">
    <source media="(max-width: 480px)" srcset="https://static.rerun.io/re_perf_telemetry_sdk_jaeger/2a32ca041640e2902bf70164a42a1b539d7d759b/480w.png">
    <source media="(max-width: 768px)" srcset="https://static.rerun.io/re_perf_telemetry_sdk_jaeger/2a32ca041640e2902bf70164a42a1b539d7d759b/768w.png">
    <source media="(max-width: 1024px)" srcset="https://static.rerun.io/re_perf_telemetry_sdk_jaeger/2a32ca041640e2902bf70164a42a1b539d7d759b/1024w.png">
    <source media="(max-width: 1200px)" srcset="https://static.rerun.io/re_perf_telemetry_sdk_jaeger/2a32ca041640e2902bf70164a42a1b539d7d759b/1200w.png">
  </picture>

* Example of in-process profiling using Tracy:
  ```sh
  # Build the SDK with performance telemetry enabled, in the 'examples' environment:
  $ py-build-perf-examples

  # Run your script with both telemetry and the Tracy integration enabled:
  $ TELEMETRY_ENABLED=true TRACY_ENABLED=true <your_script>
  ```
  <picture>
    <img src="https://static.rerun.io/re_perf_telemetry_sdk_tracy/7787342837a61d8dd85ce9174a820d5884048f9b/full.png" alt="">
    <source media="(max-width: 480px)" srcset="https://static.rerun.io/re_perf_telemetry_sdk_tracy/7787342837a61d8dd85ce9174a820d5884048f9b/480w.png">
    <source media="(max-width: 768px)" srcset="https://static.rerun.io/re_perf_telemetry_sdk_tracy/7787342837a61d8dd85ce9174a820d5884048f9b/768w.png">
    <source media="(max-width: 1024px)" srcset="https://static.rerun.io/re_perf_telemetry_sdk_tracy/7787342837a61d8dd85ce9174a820d5884048f9b/1024w.png">
    <source media="(max-width: 1200px)" srcset="https://static.rerun.io/re_perf_telemetry_sdk_tracy/7787342837a61d8dd85ce9174a820d5884048f9b/1200w.png">
  </picture>


#### Future work

* Integration with [datafusion-tracing](https://github.com/datafusion-contrib/datafusion-tracing)


### Rerun Viewer

In the Rerun Viewer, `re_perf_telemetry` is always disabled by default (feature flagged), and only meant as a developer tool: it never ships with the final user builds.

The integration only really works with in-process profiling, and even then with caveats (see `Limitations` and `Future work` below).

* Example of in-process profiling using Tracy:
  ```sh
  # Start the viewer with both telemetry and the Tracy integration enabled:
  $ TELEMETRY_ENABLED=true TRACY_ENABLED=true pixi run rerun-perf
  ```
  <picture>
    <img src="https://static.rerun.io/re_perf_telemetry_viewer_tracy/d6dbfe38d753ff550646a52f17d71942a3b27d6d/full.png" alt="">
    <source media="(max-width: 480px)" srcset="https://static.rerun.io/re_perf_telemetry_viewer_tracy/d6dbfe38d753ff550646a52f17d71942a3b27d6d/480w.png">
    <source media="(max-width: 768px)" srcset="https://static.rerun.io/re_perf_telemetry_viewer_tracy/d6dbfe38d753ff550646a52f17d71942a3b27d6d/768w.png">
    <source media="(max-width: 1024px)" srcset="https://static.rerun.io/re_perf_telemetry_viewer_tracy/d6dbfe38d753ff550646a52f17d71942a3b27d6d/1024w.png">
    <source media="(max-width: 1200px)" srcset="https://static.rerun.io/re_perf_telemetry_viewer_tracy/d6dbfe38d753ff550646a52f17d71942a3b27d6d/1200w.png">
  </picture>
  *In this screenshot, I browsed a Redap catalog and then opened one of the recordings. Can you guess when the video decoding happened? 😁*


#### Limitations

* `puffin` spans are not forwarded to the perf telemetry tools
  While this is technically do-able (for instance by replacing `puffin` calls with [the `profiling` crate](https://crates.io/crates/profiling)),
  our `puffin` spans were not implemented with the kind of overhead that the `tracing` involves in mind anyway.
  A possible future approach would be a native Tracy integration for the Viewer, see `Future work` below.

* The viewer will crash during shutdown when perf telemetry is enabled
  This is actually unrelated to `re_perf_telemetry` AFAICT: the viewer will crash on exit because of what appears to be a design flaw in `tracing-subscriber`'s shutdown implementation, specifically it assumes that all the relevant thread-local state will be dropped in the proper order, when really it won't and there's no way to guarantee that. See <https://github.com/tokio-rs/tracing/issues/3239>.
  Since this is a very niche feature only meant to be used for deep performance work, I think this is fine for now (and I don't think there's anything we can do from userspace anyhow, this is a pure `tracing` vs. Rust's TLS implementation issue).


#### Future work

The Rerun Viewer would greatly benefit from a _native_ Tracy integration (i.e. using the Tracy client directly, instead of going through the `tracing` ecosystem).

This would not only alleviate the overhead of the `tracing` ecosystem, it would also make it possible to use all the more advanced features of Tracy in the viewer (e.g. GPU spans, framebuffer previews, allocation tracing, contention spans, plots, sub-frames, etc).
Check out this [web demo](https://tracy.nereid.pl/) for a taste of what a native Tracy integration can do.
