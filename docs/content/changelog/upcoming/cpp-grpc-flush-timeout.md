---
title: C++ gRPC flush timeout
hidden: true
type: misc
---

### C++: configurable gRPC flush timeout

`RecordingStream::connect_grpc` and `GrpcSink` take a `flush_timeout` (a `std::chrono` duration), which bounds how long a flush waits for the connection to be established.
It defaults to 5 seconds, as before.
In the C API, `rr_recording_stream_connect_grpc` and `rr_grpc_sink` take the same parameter as `flush_timeout_sec`.
