# re_async

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

Async runtime abstractions for native and WebAssembly targets.

`AsyncRuntimeHandle` wraps an executor supplied by the application.
Library code should accept a handle instead of creating a Tokio runtime.
Dedicated runtimes belong at process or thread ownership boundaries.
