use opentelemetry::Context;
use opentelemetry::propagation::{Extractor, Injector, TextMapPropagator};
use opentelemetry::trace::TraceContextExt as _;

/// A propagator wrapper that adds custom tracestate key-value pairs
#[derive(Debug, Clone)]
pub struct EnrichingTraceStatePropagator {
    inner: opentelemetry_sdk::propagation::TraceContextPropagator,
    additional_entries: Vec<(String, String)>,
}
impl EnrichingTraceStatePropagator {
    pub fn new(tracestate_str: &str) -> anyhow::Result<Self> {
        let additional_entries = if tracestate_str.is_empty() {
            Vec::new()
        } else {
            tracing::info!(
                "TRACE_DEBUG Using additional tracestate entries from OTEL_PROPAGATORS_TRACESTATE: {tracestate_str}"
            );
            Self::parse_pairs(tracestate_str)?
        };

        Ok(Self {
            inner: opentelemetry_sdk::propagation::TraceContextPropagator::new(),
            additional_entries,
        })
    }

    fn parse_pairs(input: &str) -> anyhow::Result<Vec<(String, String)>> {
        input
            .split(',')
            .map(|pair| {
                let pair = pair.trim();
                if pair.is_empty() {
                    anyhow::bail!("empty tracestate pair for input {input}");
                }

                let mut parts = pair.splitn(2, '=');
                match (parts.next(), parts.next()) {
                    (Some(k), Some(v)) if !k.is_empty() && !v.is_empty() => {
                        Ok((k.trim().to_owned(), v.trim().to_owned()))
                    }
                    _ => anyhow::bail!("invalid tracestate pair format: '{pair}'"),
                }
            })
            .collect()
    }
}

impl TextMapPropagator for EnrichingTraceStatePropagator {
    fn inject_context(&self, cx: &Context, injector: &mut dyn Injector) {
        tracing::info!(
            "TRACE_DEBUG Injecting context with additional tracestate entries: {:?}",
            self.additional_entries
        );
        // first let the standard propagator inject traceparent
        self.inner.inject_context(cx, injector);

        // now enrich the tracestate if we have additional entries
        if !self.additional_entries.is_empty() {
            let span = cx.span();
            let span_context = span.span_context();
            if span_context.is_valid() {
                let mut trace_state = span_context.trace_state().clone();

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
        }
    }

    fn extract_with_context(&self, cx: &Context, extractor: &dyn Extractor) -> Context {
        self.inner.extract_with_context(cx, extractor)
    }

    fn fields(&self) -> opentelemetry::propagation::text_map_propagator::FieldIter<'_> {
        self.inner.fields()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pairs() {
        let result = EnrichingTraceStatePropagator::parse_pairs("my_id=id123,env=prod").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], ("my_id".to_owned(), "id123".to_owned()));
        assert_eq!(result[1], ("env".to_owned(), "prod".to_owned()));
    }

    #[test]
    fn test_empty_string() {
        let propagator = EnrichingTraceStatePropagator::new("").unwrap();
        assert!(propagator.additional_entries.is_empty());
    }

    #[test]
    fn test_invalid_format() {
        assert!(EnrichingTraceStatePropagator::parse_pairs("invalid").is_err());
        assert!(EnrichingTraceStatePropagator::parse_pairs("key=").is_err());
        assert!(EnrichingTraceStatePropagator::parse_pairs("=value").is_err());
    }
}
