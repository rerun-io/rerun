use std::collections::HashMap;

use opentelemetry::Context;
use opentelemetry::propagation::{Extractor, Injector, TextMapPropagator};
use opentelemetry::trace::TraceContextExt as _;

/// Propagator that enriches the outbound `tracestate` header with the active
/// `rerun_session_id`, if any.
///
/// The id source is resolved on every injection by
/// [`crate::current_rerun_session_id`] (the active Rust `with_tracing_session`
/// scope, or whatever the registered `SessionIdReader` returns — e.g. the
/// Python `tracing_session()` `ContextVar` when `rerun_py` is in use). When no
/// scope is active, this propagator is a no-op.
///
/// Registered alongside `TraceContextPropagator` in the global propagator stack;
/// runs after it, so the existing tracestate (if any) is preserved and merged.
#[derive(Debug, Default, Clone)]
pub struct TraceStateEnricher;

impl TextMapPropagator for TraceStateEnricher {
    fn inject_context(&self, cx: &Context, injector: &mut dyn Injector) {
        let Some(session_id) = crate::current_rerun_session_id() else {
            return;
        };

        let span = cx.span();
        let span_context = span.span_context();
        if !span_context.is_valid() {
            return;
        }

        let trace_state = span_context.trace_state().clone();
        let trace_state = trace_state
            .insert(crate::RERUN_SESSION_TRACESTATE_KEY.to_owned(), session_id)
            .unwrap_or(trace_state);

        let header = trace_state.header();
        if !header.is_empty() {
            injector.set("tracestate", header);
        }
    }

    fn extract_with_context(&self, cx: &Context, _extractor: &dyn Extractor) -> Context {
        // Don't modify extraction - let `TraceContextPropagator` handle it.
        cx.clone()
    }

    fn fields(&self) -> opentelemetry::propagation::text_map_propagator::FieldIter<'_> {
        static FIELDS: &[String] = &[];
        opentelemetry::propagation::text_map_propagator::FieldIter::new(FIELDS)
    }
}

/// `SpanProcessor` that decorates **root spans** (those with no parent in the
/// `OTel` `Context`) with the active `rerun_session_id`, when one is set via
/// `tracing_session()`.
///
/// Complement to [`TraceStateEnricher`]:
///
/// - [`TraceStateEnricher`] writes the id into the outbound W3C `tracestate`
///   header, so the *server side* can extract it.
/// - [`RerunSessionRootSpanProcessor`] writes the id as a span attribute on
///   the local span at creation, so Tempo queries like
///   `{ .rerun_session_id = "rs_…" }` can find *client-side* spans.
///
/// Both read from [`crate::current_rerun_session_id`] and use the same
/// [`crate::RERUN_SESSION_TRACESTATE_KEY`].
///
/// **Only registered on the Rerun-frontend OTLP path** (the `rerun://` /
/// `rerun+http(s)://` schemes — see `Telemetry::init`). Vanilla OTLP
/// destinations (Jaeger, generic collectors) get untagged spans.
///
/// **Why only root spans:** child spans share their root's `trace_id`, so
/// once Tempo finds the trace by the attribute on the root, the entire tree
/// is reachable from the trace view. Tagging every span would be redundant
/// and bloat attribute storage. Matches how the server side tags its
/// `<request>` span only (see `GrpcMakeSpan` in `grpc.rs`).
#[derive(Debug)]
pub(crate) struct RerunSessionRootSpanProcessor;

impl opentelemetry_sdk::trace::SpanProcessor for RerunSessionRootSpanProcessor {
    fn on_start(&self, span: &mut opentelemetry_sdk::trace::Span, cx: &opentelemetry::Context) {
        use opentelemetry::trace::{Span as _, TraceContextExt as _};

        // Root spans only: skip when the context already carries a valid
        // parent span (the new span will be a child of that one).
        if cx.span().span_context().is_valid() {
            return;
        }

        if let Some(id) = crate::current_rerun_session_id() {
            span.set_attribute(opentelemetry::KeyValue::new(
                crate::RERUN_SESSION_TRACESTATE_KEY,
                id.to_string(),
            ));
        }
    }

    fn on_end(&self, _span: opentelemetry_sdk::trace::SpanData) {}

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(
        &self,
        _timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        Ok(())
    }

    fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        Ok(())
    }
}

/// Parse `tracestate` pairs, keeping only valid pairs and ignoring malformed ones as
/// per W3C spec guidance. We should never fail a request because of malformed tracestate.
pub fn parse_pairs(input: &str) -> HashMap<String, String> {
    if input.is_empty() {
        return HashMap::default();
    }

    input
        .split(',')
        .filter_map(|pair| {
            let pair = pair.trim();
            if pair.is_empty() {
                return None;
            }

            let mut parts = pair.splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some(k), Some(v)) if !k.is_empty() && !v.is_empty() => {
                    Some((k.trim().to_owned(), v.trim().to_owned()))
                }
                _ => {
                    tracing::debug!("Ignoring malformed tracestate pair: '{}'", pair);
                    None
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use opentelemetry::trace::{
        SpanContext, SpanId, TraceContextExt as _, TraceFlags, TraceId, TraceState,
    };

    use super::*;

    /// Test injector that records all `set` calls into a map.
    #[derive(Default)]
    struct MapInjector {
        entries: HashMap<String, String>,
    }

    impl Injector for MapInjector {
        fn set(&mut self, key: &str, value: String) {
            self.entries.insert(key.to_owned(), value);
        }
    }

    fn ctx_with_state(state: TraceState) -> Context {
        let span_cx = SpanContext::new(
            TraceId::from_bytes([
                0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
                0xcd, 0xef,
            ]),
            SpanId::from_bytes([0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]),
            TraceFlags::SAMPLED,
            false,
            state,
        );
        Context::new().with_remote_span_context(span_cx)
    }

    #[test]
    fn enricher_no_session_is_noop() {
        let cx = ctx_with_state(TraceState::default());
        let mut injector = MapInjector::default();

        // No active tracing session in this test → `current_rerun_session_id` returns None.
        TraceStateEnricher.inject_context(&cx, &mut injector);

        assert!(
            injector.entries.is_empty(),
            "expected no header writes, got {:?}",
            injector.entries
        );
    }

    #[test]
    fn enricher_invalid_span_context_is_noop() {
        // Default Context has no valid span context.
        let cx = Context::new();
        let mut injector = MapInjector::default();

        TraceStateEnricher.inject_context(&cx, &mut injector);

        assert!(injector.entries.is_empty());
    }

    /// Build a tracer wired up with `RerunSessionRootSpanProcessor` and an
    /// in-memory exporter behind a `SimpleSpanProcessor`. The two processors
    /// fire in order (registration order), so `on_start` has run before the
    /// span is exported and its attributes are visible on `SpanData`.
    fn build_tracer_with_processor() -> (
        opentelemetry_sdk::trace::Tracer,
        opentelemetry_sdk::trace::InMemorySpanExporter,
        opentelemetry_sdk::trace::SdkTracerProvider,
    ) {
        use opentelemetry::trace::TracerProvider as _;
        let exporter = opentelemetry_sdk::trace::InMemorySpanExporter::default();
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_span_processor(RerunSessionRootSpanProcessor)
            .with_span_processor(opentelemetry_sdk::trace::SimpleSpanProcessor::new(
                exporter.clone(),
            ))
            .build();
        let tracer = provider.tracer("test");
        (tracer, exporter, provider)
    }

    fn rerun_session_attr(span: &opentelemetry_sdk::trace::SpanData) -> Option<String> {
        span.attributes
            .iter()
            .find(|kv| kv.key.as_str() == crate::RERUN_SESSION_TRACESTATE_KEY)
            .map(|kv| kv.value.to_string())
    }

    #[test]
    fn processor_no_session_root_has_no_attribute() {
        use opentelemetry::trace::Tracer as _;
        let (tracer, exporter, provider) = build_tracer_with_processor();

        // No `tracing_session()` scope active in this test build → the processor
        // sees `current_rerun_session_id() == None` and skips the attribute.
        let span = tracer.start("root");
        drop(span);
        provider.force_flush().ok();

        let spans = exporter.get_finished_spans().unwrap();
        assert_eq!(spans.len(), 1);
        assert!(
            rerun_session_attr(&spans[0]).is_none(),
            "expected no rerun_session_id on root: got {:?}",
            spans[0].attributes,
        );
    }

    #[test]
    fn processor_no_session_child_has_no_attribute() {
        use opentelemetry::trace::Tracer as _;
        let (tracer, exporter, provider) = build_tracer_with_processor();

        // Attach a parent context with a valid (remote) span. The processor's
        // first check (`cx.span().span_context().is_valid()`) is true here, so
        // it returns early without touching attributes — independent of whether
        // a session id is set.
        let parent_cx = ctx_with_state(TraceState::default());
        let _attach = parent_cx.attach();
        let span = tracer.start("child");
        drop(span);
        drop(_attach);
        provider.force_flush().ok();

        let spans = exporter.get_finished_spans().unwrap();
        assert_eq!(spans.len(), 1);
        assert!(
            rerun_session_attr(&spans[0]).is_none(),
            "expected no rerun_session_id on child: got {:?}",
            spans[0].attributes,
        );
    }

    #[test]
    fn processor_active_session_root_sets_attribute() {
        use opentelemetry::trace::Tracer as _;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let sid = crate::tracing_session::RerunTracingSessionId::parse("rs_cafebabe").unwrap();
        let expected = sid.to_string();
        let (tracer, exporter, provider) = build_tracer_with_processor();

        rt.block_on(crate::tracing_session::scope_session_id_for_test(
            Some(sid),
            async {
                let span = tracer.start("root");
                drop(span);
            },
        ));
        provider.force_flush().ok();

        let spans = exporter.get_finished_spans().unwrap();
        assert_eq!(spans.len(), 1);
        let attr = rerun_session_attr(&spans[0]).unwrap_or_else(|| {
            panic!(
                "expected rerun_session_id on root span: got {:?}",
                spans[0].attributes,
            )
        });
        assert_eq!(attr, expected);
    }

    #[test]
    fn test_parse_pairs_resilient() {
        // Valid pairs
        let result = parse_pairs("my_id=id123,env=prod");
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("my_id"), Some(&"id123".to_owned()));
        assert_eq!(result.get("env"), Some(&"prod".to_owned()));

        // Mixed valid and invalid pairs - keeps only valid ones
        let result = parse_pairs("valid=ok,invalid,key=,=value,also_valid=good");
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("valid"), Some(&"ok".to_owned()));
        assert_eq!(result.get("also_valid"), Some(&"good".to_owned()));

        // Empty string
        let result = parse_pairs("");
        assert!(result.is_empty());

        // All invalid pairs
        let result = parse_pairs("invalid,key=,=value");
        assert!(result.is_empty());
    }
}
