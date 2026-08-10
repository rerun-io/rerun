---
title: "Multi-sink support for GrpcServerSink"
hidden: true
type: highlight
---

### Multi-sink support for `GrpcServerSink`

`GrpcServerSink` can now be combined with other recording sinks in the Rust, Python, and C++ SDKs.
This makes it possible to stream data to connected Viewers while simultaneously writing the same recording elsewhere.

```python
# Stream data to several sinks at once:
rr.set_sinks(
    # Host a gRPC proxy server that web Viewers can connect to:
    rr.GrpcServerSink(),
    # Write data to a `data.rrd` file in the current directory:
    rr.FileSink("data.rrd"),
)
```

See [Multiple sinks](../concepts/logging-and-ingestion/sinks.md#multiple-sinks-tee-pattern) for more information.
