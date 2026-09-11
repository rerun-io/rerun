//! In-process server doubles for tests that need to capture outbound
//! OTel/PostHog traffic from production code.

pub mod assert;
pub mod otlp;
pub mod posthog;
