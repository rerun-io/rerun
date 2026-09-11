//! Test assertion macros for the mock sinks in this crate.

/// Assert that a mock sink has received no requests.
///
/// Works on any type that exposes `fn received(&self) -> Vec<T>` where `T: Debug`,
/// e.g. [`crate::otlp::MockOtlpCollector`] or [`crate::posthog::MockPostHog`].
/// Panics with the contents of the buffer if any requests are present, so failures
/// show exactly what arrived unexpectedly.
///
/// Pass by reference: `assert_sink_empty!(&collector)`.
#[macro_export]
macro_rules! assert_sink_empty {
    ($sink:expr $(,)?) => {{
        let __received = $sink.received();
        assert!(
            __received.is_empty(),
            "expected empty, got {} request(s):\n{:#?}",
            __received.len(),
            __received,
        );
    }};
}
