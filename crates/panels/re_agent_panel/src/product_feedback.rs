//! Feedback that an agent sends about Rerun, through the `submit_feedback` viewer control
//! operation, which `re_viewer_mcp` offers agents as an MCP tool.

use re_analytics::event::{
    Feedback, FeedbackAbout, FeedbackAuthor, FeedbackCategory, FeedbackSeverity,
    ProductFeedbackContent,
};
use re_protos::viewer_control::v1alpha1::{
    FeedbackCategory as ProtoCategory, FeedbackSeverity as ProtoSeverity, SubmitFeedbackRequest,
};
use re_sdk_types::external::uuid::Uuid;

const FEEDBACK_SOURCE: &str = "mcp";

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct RedactableContent {
    title: String,
    summary: Option<String>,
}

/// Records feedback without its title and summary, which may hold sensitive data.
pub fn record_without_text(request: SubmitFeedbackRequest) {
    let (unsent, _text) = Unsent::from_request(request);
    unsent.record(None);
}

/// Sends feedback, with its text once a second session of the panel's agent has redacted it.
#[derive(Default)]
pub struct ProductFeedback {
    redactor: re_agent::Redactor,
}

impl ProductFeedback {
    /// Drive the redaction. Call every frame.
    pub fn update(&mut self, egui_ctx: &egui::Context) {
        crate::agent_analytics::update_redactor(&mut self.redactor, egui_ctx);
    }

    pub fn submit(
        &mut self,
        request: SubmitFeedbackRequest,
        redact_with: Option<re_agent::RedactionAgent>,
    ) {
        let Some(agent) = redact_with else {
            record_without_text(request);
            return;
        };
        let (unsent, text) = Unsent::from_request(request);

        let json = match serde_json::to_value(&text) {
            Ok(json) => json,
            Err(err) => {
                re_log::warn!("The text of an agent's feedback on Rerun was not sent: {err}");
                unsent.record(None);
                return;
            }
        };

        re_log::info!(
            "An agent sent feedback on Rerun. Removing sensitive data from it. It is sent after that."
        );
        self.redactor.redact(
            json,
            agent,
            Box::new(move |redacted| {
                let text = match redacted {
                    Ok(json) => serde_json::from_value::<RedactableContent>(json)
                        .map_err(|err| err.to_string()),
                    Err(err) => Err(err.to_string()),
                };
                match text {
                    Ok(text) => unsent.record(Some(text)),
                    Err(err) => {
                        re_log::warn!(
                            "The text of an agent's feedback on Rerun was not sent: the sensitive data could not be removed ({err})."
                        );
                        unsent.record(None);
                    }
                }
            }),
        );
    }
}

struct Unsent {
    agent: String,
    category: Option<FeedbackCategory>,
    severity: Option<FeedbackSeverity>,
    surface: Option<String>,
}

impl Unsent {
    fn from_request(request: SubmitFeedbackRequest) -> (Self, RedactableContent) {
        let SubmitFeedbackRequest {
            title,
            category,
            surface,
            summary,
            agent,
            severity,
        } = request;
        let category = self::category(ProtoCategory::try_from(category).unwrap_or_default());
        let severity = self::severity(ProtoSeverity::try_from(severity).unwrap_or_default());
        let unsent = Self {
            agent: agent.unwrap_or_else(|| "unknown".to_owned()),
            category,
            severity,
            surface,
        };
        (unsent, RedactableContent { title, summary })
    }

    fn event(self, content: Option<RedactableContent>) -> Feedback {
        let Self {
            agent,
            category,
            severity,
            surface,
        } = self;
        Feedback {
            feedback_id: Uuid::new_v4(),
            author: FeedbackAuthor::Agent,
            source: FEEDBACK_SOURCE,
            agent,
            about: FeedbackAbout::Product(re_analytics::event::ProductFeedback {
                category,
                severity,
                surface,
                content: content.map(|RedactableContent { title, summary }| {
                    ProductFeedbackContent { title, summary }
                }),
            }),
        }
    }

    fn record(self, text: Option<RedactableContent>) {
        let with_text = text.is_some();
        re_analytics::record(|| self.event(text));
        if with_text {
            re_log::info!("An agent's feedback on Rerun was sent.");
        } else {
            re_log::info!("An agent's feedback on Rerun was sent, without its text.");
        }
    }
}

fn category(category: ProtoCategory) -> Option<FeedbackCategory> {
    match category {
        ProtoCategory::Unspecified => None,
        ProtoCategory::Bug => Some(FeedbackCategory::Bug),
        ProtoCategory::DocsMismatch => Some(FeedbackCategory::DocsMismatch),
        ProtoCategory::Friction => Some(FeedbackCategory::Friction),
        ProtoCategory::FeatureGap => Some(FeedbackCategory::FeatureGap),
        ProtoCategory::QualityDegradation => Some(FeedbackCategory::QualityDegradation),
        ProtoCategory::Other => Some(FeedbackCategory::Other),
    }
}

fn severity(severity: ProtoSeverity) -> Option<FeedbackSeverity> {
    match severity {
        ProtoSeverity::Unspecified => None,
        ProtoSeverity::Critical => Some(FeedbackSeverity::Critical),
        ProtoSeverity::High => Some(FeedbackSeverity::High),
        ProtoSeverity::Medium => Some(FeedbackSeverity::Medium),
        ProtoSeverity::Low => Some(FeedbackSeverity::Low),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SubmitFeedbackRequest {
        SubmitFeedbackRequest {
            title: "`rerun_set_blueprint` rejected a valid blueprint.".to_owned(),
            category: ProtoCategory::Bug.into(),
            surface: Some("rerun_set_blueprint".to_owned()),
            summary: Some("It said the JSON was invalid.".to_owned()),
            agent: Some("Test Agent".to_owned()),
            severity: ProtoSeverity::Unspecified.into(),
        }
    }

    #[test]
    fn without_redaction_the_text_is_left_out() {
        let (unsent, _) = Unsent::from_request(request());
        let Feedback { agent, about, .. } = unsent.event(None);
        assert_eq!(agent, "Test Agent");
        let FeedbackAbout::Product(product) = about else {
            panic!("expected product feedback");
        };
        assert_eq!(product.category, Some(FeedbackCategory::Bug));
        assert_eq!(product.severity, None);
        assert_eq!(product.surface.as_deref(), Some("rerun_set_blueprint"));
        assert!(product.content.is_none());
    }

    #[test]
    fn redacted_content_is_sent() {
        let (unsent, text) = Unsent::from_request(request());
        let FeedbackAbout::Product(product) = unsent.event(Some(text)).about else {
            panic!("expected product feedback");
        };
        let content = product.content.expect("content");
        assert_eq!(
            content.title,
            "`rerun_set_blueprint` rejected a valid blueprint."
        );
        assert_eq!(
            content.summary.as_deref(),
            Some("It said the JSON was invalid.")
        );
    }

    /// The redactor must get back the same shape it was given.
    #[test]
    fn redactable_text_round_trips_through_json() {
        let text = RedactableContent {
            title: "Docs for `rr.log` are wrong.".to_owned(),
            summary: Some("The example uses a removed argument.".to_owned()),
        };
        let json = serde_json::to_value(&text).expect("serialize");
        assert_eq!(
            serde_json::from_value::<RedactableContent>(json).expect("deserialize"),
            text
        );
    }
}
