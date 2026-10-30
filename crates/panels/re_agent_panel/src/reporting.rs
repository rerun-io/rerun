//! What the agent panel sends to Rerun: feedback on a conversation, feedback that an agent sends
//! about Rerun, and shared prompts. Text is only sent after a second session of the panel's own
//! agent has redacted it.

use re_agent::TurnReport;
use re_agent_ui::AgentPanel;

use crate::feedback;

/// The feedback modal, and the queues of what waits for redaction.
#[derive(Default)]
pub struct Reporting {
    feedback: feedback::FeedbackModal,

    /// Feedback that agents send about Rerun through the `submit_feedback` viewer control operation.
    product_feedback: crate::product_feedback::ProductFeedback,

    analytics: crate::agent_analytics::TurnAnalytics,
}

impl Reporting {
    /// The feedback modal, after the panel's own `ui`. Call every frame.
    pub fn ui(&mut self, panel: &AgentPanel, ui: &egui::Ui) {
        let Self {
            feedback,
            product_feedback,
            analytics,
        } = self;
        if panel.feedback_requested() {
            feedback.open(panel.session(), panel.redaction_agent().ok());
        }
        feedback.ui(ui.ctx());
        product_feedback.update(ui.ctx());
        analytics.update(ui.ctx());
    }

    /// Queue feedback that an agent sent about Rerun, to be sent.
    ///
    /// Its text is only sent when `share_text` is set, after `panel`'s agent has redacted it.
    pub fn submit_product_feedback(
        &mut self,
        panel: Option<&AgentPanel>,
        request: re_protos::viewer_control::v1alpha1::SubmitFeedbackRequest,
        share_text: bool,
    ) {
        let redact_with = if share_text {
            match panel.map(AgentPanel::redaction_agent) {
                Some(Ok(agent)) => Some(agent),
                Some(Err(err)) => {
                    re_log::warn!("The text of an agent's feedback on Rerun is not sent: {err}");
                    None
                }
                None => None,
            }
        } else {
            None
        };
        self.product_feedback.submit(request, redact_with);
    }

    /// Record non-content usage for every turn and, when enabled, share its redacted text.
    pub fn record_finished_turns(&mut self, panel: &AgentPanel, turns: Vec<TurnReport>) {
        // Redaction spawns an agent session per turn, so it must not run when the
        // event it prepares would be dropped anyway.
        let share_redacted_prompts =
            panel.settings().share_redacted_prompts && re_analytics::is_enabled();
        for turn in turns {
            crate::agent_analytics::record_usage(&turn);
            if share_redacted_prompts {
                self.analytics.queue(turn, panel.redaction_agent().ok());
            }
        }
        if !share_redacted_prompts {
            // The user may have just unticked the box: drop what is still queued.
            self.analytics.discard_pending();
        }
    }
}
