use opentelemetry::Context;
use opentelemetry::propagation::{Extractor, Injector, TextMapPropagator};
use opentelemetry::trace::TraceContextExt as _;

/// A propagator that enriches `tracestate` with additional key-value pairs
#[derive(Debug, Clone)]
pub struct TraceStateEnricher {
    additional_entries: Vec<(String, String)>,
}

impl TraceStateEnricher {
    pub fn new(tracestate_str: &str) -> anyhow::Result<Self> {
        Ok(Self {
            additional_entries: parse_pairs(tracestate_str)?,
        })
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

pub fn parse_pairs(input: &str) -> anyhow::Result<Vec<(String, String)>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pairs() {
        let result = parse_pairs("my_id=id123,env=prod").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], ("my_id".to_owned(), "id123".to_owned()));
        assert_eq!(result[1], ("env".to_owned(), "prod".to_owned()));
    }

    #[test]
    fn test_empty_string() {
        let result = parse_pairs("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_invalid_format() {
        assert!(parse_pairs("invalid").is_err());
        assert!(parse_pairs("key=").is_err());
        assert!(parse_pairs("=value").is_err());
        assert!(parse_pairs("key1=value1,,key2=value2").is_err());
    }
}
