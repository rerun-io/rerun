//! Sends finished agent turns as analytics, after a fresh agent session has removed
//! sensitive data from them.

mod redactor;

use re_agent_ui::{LaunchConfig, TurnReport};

use self::redactor::RedactionError;
pub use self::redactor::Redactor;

/// Used for text that could not be redacted. The reason follows the colon.
const REDACTION_FAILED: &str = "<redaction failed";

/// The free-text parts of a turn report: what the redacting agent gets to rewrite.
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct RedactableText {
    prompt: String,
    response: String,
    failed_tool_calls: Vec<String>,
    errors: Vec<String>,
}

impl RedactableText {
    fn from_report(report: &TurnReport) -> Self {
        Self {
            prompt: report.prompt.clone(),
            response: report.response.clone(),
            failed_tool_calls: report.failed_tool_calls.clone(),
            errors: report.errors.clone(),
        }
    }

    /// Stand-in text for a turn that reached analytics without being redacted.
    ///
    /// `reason` is recorded too: every one of them is a bug or a broken agent, and the
    /// placeholder is the only place it can be seen from.
    fn failed(reason: &str) -> Self {
        let text = format!("{REDACTION_FAILED}: {reason}>");
        Self {
            prompt: text.clone(),
            response: text,
            failed_tool_calls: Vec::new(),
            errors: Vec::new(),
        }
    }
}

/// Queues turn reports for redaction, and records each one once it is redacted.
#[derive(Default)]
pub struct TurnAnalytics {
    redactor: Redactor,
}

impl TurnAnalytics {
    /// Queue a turn whose text the user has agreed to share.
    ///
    /// Only call this while sharing is on: a queued turn is always recorded, with the text
    /// replaced by the reason when the redactor could not produce any. `config` is the panel's
    /// own launch config, or `None` when the panel could not produce one, which is itself one
    /// of those reasons.
    pub fn queue(&mut self, report: TurnReport, config: Option<LaunchConfig>) {
        let Some(config) = config else {
            record(&report, RedactableText::failed("no redactor"));
            return;
        };
        let Ok(json) = serde_json::to_value(RedactableText::from_report(&report)) else {
            record(&report, RedactableText::failed("not serializable"));
            return;
        };
        self.redactor.redact(
            json,
            config,
            Box::new(move |redacted| {
                let text = redacted
                    .and_then(|json| {
                        serde_json::from_value(json).map_err(|err| {
                            re_log::debug!("Redacted analytics are not a turn's text: {err}");
                            RedactionError::UnusableAnswer
                        })
                    })
                    .unwrap_or_else(|err| RedactableText::failed(&err.to_string()));
                record(&report, text);
            }),
        );
    }

    /// Stop redaction and discard the reports that have not been sent.
    ///
    /// Dropping the sender ends the thread, so this is free to call on every frame that
    /// sharing is off.
    pub fn discard_pending(&mut self) {
        self.redactor.discard_pending();
    }
}

/// Record non-content usage statistics for a finished turn.
pub fn record_usage(report: &TurnReport) {
    re_analytics::record(|| usage_from_report(report));
}

fn usage_from_report(report: &TurnReport) -> re_analytics::event::AgentTurnUsage {
    re_analytics::event::AgentTurnUsage {
        agent: report.agent.clone(),
        duration_secs: report.duration.as_secs_f64(),
        outcome: report.outcome.name().to_owned(),
        tool_calls: report.tool_calls,
        tokens_used: report.tokens_used,
        token_limit: report.token_limit,
        failed_tool_calls: u32::try_from(report.failed_tool_calls.len()).unwrap_or(u32::MAX),
        errors: u32::try_from(report.errors.len()).unwrap_or(u32::MAX),
        permissions_requested: report.permissions_requested,
        permissions_rejected: report.permissions_rejected,
    }
}

fn record(report: &TurnReport, text: RedactableText) {
    let usage = usage_from_report(report);
    let RedactableText {
        prompt,
        response,
        failed_tool_calls,
        errors,
    } = text;
    re_analytics::record(|| re_analytics::event::AgentTurn {
        usage,
        prompt,
        response,
        failed_tool_calls,
        errors,
    });
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn redactable_text_carries_the_reports_text() {
        let report = TurnReport {
            agent: None,
            prompt: "hello".to_owned(),
            duration: Duration::ZERO,
            outcome: re_agent_ui::TurnOutcome::Error,
            response: String::new(),
            tool_calls: 0,
            tokens_used: Some(123),
            token_limit: Some(456),
            failed_tool_calls: Vec::new(),
            off_limits_paths: Vec::new(),
            errors: vec!["boom".to_owned()],
            permissions_requested: 0,
            permissions_rejected: 0,
        };
        let text = RedactableText::from_report(&report);
        assert_eq!(text.prompt, "hello");
        assert_eq!(text.errors, vec!["boom"]);
    }
}
