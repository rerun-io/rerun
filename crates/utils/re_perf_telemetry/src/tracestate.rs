use std::collections::HashMap;

use opentelemetry::Context;
use opentelemetry::propagation::{Extractor, Injector, TextMapPropagator};
use opentelemetry::trace::TraceContextExt as _;

/// A propagator that enriches `tracestate` with additional key-value pairs
#[derive(Debug, Clone)]
pub struct TraceStateEnricher {
    additional_entries: Vec<(String, String)>,
}

impl TraceStateEnricher {
    pub fn new(tracestate_str: &str) -> Self {
        Self {
            additional_entries: parse_pairs(tracestate_str).into_iter().collect(),
        }
    }
}

impl TextMapPropagator for TraceStateEnricher {
    fn inject_context(&self, cx: &Context, injector: &mut dyn Injector) {
        if self.additional_entries.is_empty() {
            return;
        }

        let span = cx.span();
        let span_context = span.span_context();
        if !span_context.is_valid() {
            return;
        }

        // Start with existing `tracestate` from span context
        let mut trace_state = span_context.trace_state().clone();

        // Add our additional entries
        for (key, value) in &self.additional_entries {
            trace_state = trace_state
                .insert(key.clone(), value.clone())
                .unwrap_or(trace_state);
        }

        let header = trace_state.header();
        if !header.is_empty() {
            injector.set("tracestate", header);
        }
    }

    fn extract_with_context(&self, cx: &Context, _extractor: &dyn Extractor) -> Context {
        // Don't modify extraction - let TraceContextPropagator handle it
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
    use super::*;

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
