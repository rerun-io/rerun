# re_test_mocks

In-process server doubles (`MockOtlpCollector`, `MockPostHog`) used by tests that need to capture outbound OTel/PostHog traffic.

Both mocks are full implementations of the wire protocols they stand in for — a tonic gRPC `TraceService` for OTLP and an axum HTTP handler for PostHog's `/batch` endpoint. They run on ephemeral ports in the test process, capture every request, and expose notification-driven `wait_for(…)` and `received()` accessors so tests don't have to poll. The `assert_sink_empty!` macro is the companion no-traffic assertion.

The crate root re-exports nothing; consumers reach into the submodules directly via `re_test_mocks::otlp::MockOtlpCollector` and `re_test_mocks::posthog::MockPostHog`.
