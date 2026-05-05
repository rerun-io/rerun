use std::collections::HashMap;

use opentelemetry::Context;
use opentelemetry::propagation::{Extractor, Injector, TextMapPropagator};
use opentelemetry::trace::TraceContextExt as _;

/// Propagator that enriches the outbound `tracestate` header with the active
/// `rerun_session_id`, if any.
///
/// The id source is resolved on every injection by [`crate::current_rerun_session_id`]:
/// the Python `tracing_session()` `ContextVar` (when built with `pyo3`). When no scope
/// is active, this propagator is a no-op.
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

        // No `pyo3` ContextVar populated in this test → `current_rerun_session_id` returns None.
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
