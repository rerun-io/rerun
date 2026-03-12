use std::fmt;

use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

/// Number of hex characters to show in the text-format log prefix.
/// 8 hex chars = 4 bytes = ~4 billion unique values — plenty for local dev.
const SHORT_TRACE_ID_LEN: usize = 8;

/// A [`FormatEvent`] wrapper that injects the current `OpenTelemetry` `trace_id`
/// into every log line.
///
/// For JSON output (`is_json = true`), the full `trace_id` is injected as a
/// top-level JSON field. For text output (`is_json = false`), a short 8-char
/// prefix is prepended in brackets to keep timestamps aligned.
///
/// The `trace_id` is included regardless of whether the trace is sampled,
/// as long as the span context is valid.
pub struct TraceIdFormat<F> {
    inner: F,
    is_json: bool,
}

impl<F> TraceIdFormat<F> {
    pub fn new(inner: F, is_json: bool) -> Self {
        Self { inner, is_json }
    }
}

fn current_trace_id() -> Option<String> {
    use opentelemetry::trace::TraceContextExt as _;

    // Read directly from the OTel thread-local context, which is set by the
    // `tracing-opentelemetry` layer's `on_enter` (context activation).
    // This is more robust than going through `tracing::Span::current().context()`
    // because it doesn't depend on the tracing→otel span lookup.
    let cx = opentelemetry::Context::current();
    let span = cx.span();
    let span_cx = span.span_context();

    span_cx.is_valid().then(|| span_cx.trace_id().to_string())
}

impl<S, N, F> FormatEvent<S, N> for TraceIdFormat<F>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
    F: FormatEvent<S, N>,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> fmt::Result {
        let trace_id = current_trace_id();

        if self.is_json {
            // JSON: buffer the full output (ANSI is never used in JSON) and
            // inject `"trace_id":"…"` as the first field after the opening brace.
            let mut buf = String::with_capacity(512);
            let buf_writer = Writer::new(&mut buf);
            self.inner.format_event(ctx, buf_writer, event)?;

            if let Some(ref trace_id) = trace_id {
                if let Some(after_brace) = buf.strip_prefix('{') {
                    writer.write_str("{\"trace_id\":\"")?;
                    writer.write_str(trace_id)?;
                    writer.write_str("\",")?;
                    writer.write_str(after_brace)?;
                } else {
                    writer.write_str(&buf)?;
                }
            } else {
                writer.write_str(&buf)?;
            }
        } else {
            // Text (pretty/compact): prepend a short trace_id tag (first 8 hex
            // chars) so timestamps stay aligned. Write directly to the real
            // writer to preserve ANSI escape sequences.
            //
            // With trace context: `[a1b2c3d4] 2026-03-06 INFO …`
            // Without:            `[--------] 2026-03-06 INFO …`
            match trace_id {
                Some(ref id) => {
                    // Use `get` to avoid panicking if the id is unexpectedly short
                    // or non-ASCII. The format spec pads with `-` to keep alignment.
                    let prefix = id.get(..SHORT_TRACE_ID_LEN).unwrap_or(id.as_str());
                    write!(writer, "[{prefix:-<SHORT_TRACE_ID_LEN$}] ")?;
                }
                None => writer.write_str("[--------] ")?,
            }
            self.inner.format_event(ctx, writer, event)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use opentelemetry::trace::TracerProvider as _;
    use tracing_subscriber::Layer as _;
    use tracing_subscriber::layer::SubscriberExt as _;

    use super::*;

    /// A writer that captures output into a shared buffer.
    #[derive(Clone)]
    struct CaptureWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl CaptureWriter {
        fn new() -> Self {
            Self {
                buf: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn output(&self) -> String {
            String::from_utf8(self.buf.lock().clone()).unwrap()
        }
    }

    impl std::io::Write for CaptureWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.buf.lock().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
        type Writer = Self;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// Creates a real SDK tracer that generates valid span contexts but exports nothing.
    fn test_tracer_provider() -> opentelemetry_sdk::trace::SdkTracerProvider {
        opentelemetry_sdk::trace::SdkTracerProvider::builder().build()
    }

    fn make_subscriber_json(
        writer: CaptureWriter,
        provider: &opentelemetry_sdk::trace::SdkTracerProvider,
    ) -> impl tracing::Subscriber + Send + Sync + 'static {
        let otel_layer = tracing_opentelemetry::layer().with_tracer(provider.tracer("test"));

        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(writer)
            .with_target(false)
            .with_file(false)
            .with_line_number(false)
            .json()
            .map_event_format(|f| TraceIdFormat::new(f, true))
            .with_filter(tracing_subscriber::filter::LevelFilter::INFO);

        tracing_subscriber::registry()
            .with(otel_layer)
            .with(fmt_layer)
    }

    fn make_subscriber_compact(
        writer: CaptureWriter,
        provider: &opentelemetry_sdk::trace::SdkTracerProvider,
    ) -> impl tracing::Subscriber + Send + Sync + 'static {
        let otel_layer = tracing_opentelemetry::layer().with_tracer(provider.tracer("test"));

        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(writer)
            .with_target(false)
            .with_file(false)
            .with_line_number(false)
            .compact()
            .map_event_format(|f| TraceIdFormat::new(f, false))
            .with_filter(tracing_subscriber::filter::LevelFilter::INFO);

        tracing_subscriber::registry()
            .with(otel_layer)
            .with(fmt_layer)
    }

    #[test]
    fn json_format_includes_trace_id() {
        let provider = test_tracer_provider();
        let writer = CaptureWriter::new();
        let subscriber = make_subscriber_json(writer.clone(), &provider);

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("test_span");
            let _enter = span.enter();
            tracing::info!("hello");
        });

        let output = writer.output();
        let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        let trace_id = parsed["trace_id"].as_str();
        assert!(
            trace_id.is_some(),
            "trace_id should be a top-level JSON field: {output}"
        );
        assert_ne!(
            trace_id.unwrap(),
            "00000000000000000000000000000000",
            "trace_id should not be all zeros: {output}"
        );
    }

    #[test]
    fn json_format_no_trace_id_without_span() {
        let provider = test_tracer_provider();
        let writer = CaptureWriter::new();
        let subscriber = make_subscriber_json(writer.clone(), &provider);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hello");
        });

        let output = writer.output();
        let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert!(
            parsed.get("trace_id").is_none(),
            "trace_id should be absent without an active span: {output}"
        );
    }

    #[test]
    fn text_format_includes_short_trace_id_prefix() {
        let provider = test_tracer_provider();
        let writer = CaptureWriter::new();
        let subscriber = make_subscriber_compact(writer.clone(), &provider);

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("test_span");
            let _enter = span.enter();
            tracing::info!("hello");
        });

        let output = writer.output();
        // Should start with `[<8 hex chars>] `
        let trimmed = output.trim_start();
        assert!(
            trimmed.starts_with('['),
            "text output should start with a bracketed trace_id prefix: {output}"
        );
        let bracket_end = trimmed.find(']').expect("missing closing bracket");
        let prefix = &trimmed[1..bracket_end];
        assert_eq!(
            prefix.len(),
            SHORT_TRACE_ID_LEN,
            "trace_id prefix should be {SHORT_TRACE_ID_LEN} chars: got {prefix:?}"
        );
        assert!(
            prefix != "--------",
            "trace_id prefix should not be the placeholder: {output}"
        );
    }

    #[test]
    fn text_format_placeholder_without_span() {
        let provider = test_tracer_provider();
        let writer = CaptureWriter::new();
        let subscriber = make_subscriber_compact(writer.clone(), &provider);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hello");
        });

        let output = writer.output();
        assert!(
            output.contains("[--------]"),
            "text output should contain placeholder when no trace context: {output}"
        );
    }

    #[test]
    fn json_output_is_valid_json_with_fields() {
        let provider = test_tracer_provider();
        let writer = CaptureWriter::new();
        let subscriber = make_subscriber_json(writer.clone(), &provider);

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("test_span");
            let _enter = span.enter();
            tracing::info!(key = "value", "test message");
        });

        let output = writer.output();
        let parsed: serde_json::Value = serde_json::from_str(output.trim())
            .unwrap_or_else(|e| panic!("output should be valid JSON: {e}\noutput: {output}"));
        assert!(parsed["trace_id"].as_str().is_some());
        assert!(parsed["fields"]["message"].as_str().is_some());
    }
}
