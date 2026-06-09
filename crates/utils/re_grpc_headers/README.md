# re_grpc_headers

Rerun gRPC header conventions.

Contains the well-known `x-rerun-*` header names, the `RerunVersionInterceptor` that stamps every outbound request with the client (or server) identity and version, the matching tower `Layer` helpers that wire it into a stack, and a small fork of `tower-http::propagate_header` used to propagate multiple Rerun headers between requests and responses.

Everything here is plain `tonic`/`tower`/`http` plumbing — no rerun-internal types — so it can sit on the `crates/utils` tier and be consumed by any crate that needs the same gRPC header behavior.
