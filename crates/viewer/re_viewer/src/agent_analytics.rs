//! Sends finished agent turns as analytics, after a fresh agent session has removed
//! sensitive data from them.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crossbeam::channel::{Receiver, RecvTimeoutError, Sender, TrySendError};
use re_agent_ui::{AgentSession, LaunchConfig, TurnReport};
use tempfile::TempDir;

/// How long the redacting agent gets before the turn is sent with its text fields dropped.
const REDACTION_TIMEOUT: Duration = Duration::from_mins(2);

/// Maximum number of turns waiting for redaction before new text is dropped.
const REDACTION_QUEUE_CAPACITY: usize = 1024;

/// Used for text that could not be redacted. The reason follows the colon.
const REDACTION_FAILED: &str = "<redaction failed";

/// Models to redact with, best first, matched against whatever the user's agent offers.
///
/// Redaction is mechanical work on one short JSON object, so the cheapest capable model of each
/// family is enough, and the user should not pay flagship prices for a background task.
/// An agent that offers none of these keeps its default model.
const REDACTION_MODELS: &[&str] = &["sonnet", "flash", "mini", "haiku"];

/// Instructions for removing sensitive data from agent turn analytics.
const REDACTION_INSTRUCTIONS: &str = r#"# Redact analytics

You receive one JSON object. Return the same object with every sensitive value removed and nothing else: no explanation, no code fence, no extra keys.

## Remove

Replace each of these with the placeholder in angle brackets:

| What                                                                           | Placeholder | Examples                                                                   |
|--------------------------------------------------------------------------------|-------------|----------------------------------------------------------------------------|
| People's names, usernames, handles                                             | `<name>`    | `Ada Lovelace`, `@emilk`, `/Users/ada/` becomes `/Users/<name>/`           |
| Companies, customers, teams, products that are not Rerun                       | `<org>`     | `Acme Robotics`, `acme-prod`                                               |
| Project, dataset, robot, and vehicle names                                      | `<project>` | `warehouse-pick-v3`, `robot-07`                                            |
| Email addresses, phone numbers, postal addresses                               | `<contact>` |                                                                            |
| Hostnames, IP addresses, URLs other than rerun.io, github.com/rerun-io, and public documentation sites | `<host>` | `redap.acme.internal:51234` |
| UUIDs, hashes, recording ids, dataset ids, segment ids, ticket numbers          | `<id>`      | `1830B33B45B963E7774455beb91701ae`                                         |
| API keys, tokens, passwords, connection strings, anything that looks like a secret | `<secret>` | `hf_…`, `sk-…`, `Bearer …`                                             |
| File and directory names that carry any of the above                            | as above, keeping the extension | `/data/acme/run_42.rrd` becomes `/data/<org>/<project>.rrd` |

When unsure whether something identifies a person or an organization, redact it.

Err on the side of privacy!

## Keep

- Rerun API names, archetypes, components, entity paths that are generic (`world/camera`), CLI flags, error messages, and stack traces without paths.
- Programming-language keywords, library names, and public documentation URLs.
- Numbers that are measurements, counts, or timestamps.
- The structure: every key stays, arrays keep their length, strings that contain nothing sensitive are returned unchanged.

## Output

The redacted JSON object only, on one line or pretty-printed, starting with `{` and ending with `}`. Do not run tools, do not ask questions."#;

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

/// A report waiting on the redacting agent.
struct InFlight {
    redactor: AgentSession,
    report: TurnReport,
    started: Instant,
    prompt_sent: bool,
}

/// Work sent to the redaction thread.
struct RedactionRequest {
    report: TurnReport,
    config: LaunchConfig,
}

/// Queues turn reports on a background thread, where a fresh, tool-less agent session redacts
/// each one. Reports are handled one at a time, and nothing carries over between sessions.
#[derive(Default)]
pub struct TurnAnalytics {
    /// Declared before `scratch` so that dropping this ends the redaction thread before
    /// the directory it runs in is removed.
    requests: Option<Sender<RedactionRequest>>,

    /// An empty directory to run the redacting agents in.
    ///
    /// Never the user's project: an agent started there loads its instruction files, settings,
    /// and hooks, none of which have any business in a session that only rewrites one JSON
    /// object.
    ///
    /// Made once and shared, since the sessions run one at a time.
    scratch: Option<TempDir>,
}

impl TurnAnalytics {
    /// Queue a turn whose text the user has agreed to share.
    ///
    /// Only call this while sharing is on: a queued turn is always recorded, with the text
    /// replaced by the reason when the redactor could not produce any. `config` is the panel's
    /// own launch config, or `None` when the panel could not produce one, which is itself one
    /// of those reasons.
    pub fn queue(&mut self, report: TurnReport, config: Option<LaunchConfig>) {
        let Some(config) = config.and_then(|config| self.redactor_config(config)) else {
            record(&report, RedactableText::failed("no redactor"));
            return;
        };
        let request = RedactionRequest { report, config };
        let Some(requests) = self.requests() else {
            record(
                &request.report,
                RedactableText::failed("no redaction thread"),
            );
            return;
        };
        if let Err(err) = requests.try_send(request) {
            let (request, reason) = match err {
                TrySendError::Full(request) => {
                    re_log::warn_once!("Analytics redaction queue is full");
                    (request, "queue full")
                }
                TrySendError::Disconnected(request) => {
                    self.requests = None;
                    (request, "redaction thread gone")
                }
            };
            record(&request.report, RedactableText::failed(reason));
        }
    }

    /// Stop redaction and discard the reports that have not been sent.
    ///
    /// Dropping the sender ends the thread, so this is free to call on every frame that
    /// sharing is off.
    pub fn discard_pending(&mut self) {
        self.requests = None;
    }

    fn requests(&mut self) -> Option<&Sender<RedactionRequest>> {
        if self.requests.is_none() {
            self.requests = spawn_redaction_thread();
        }
        self.requests.as_ref()
    }

    /// A config for the redacting session: the same agent as the panel, but without MCP servers,
    /// so it has nothing but the prompt.
    ///
    /// `None` when there is no scratch directory to run it in.
    fn redactor_config(&mut self, mut config: LaunchConfig) -> Option<LaunchConfig> {
        if self.scratch.is_none() {
            match TempDir::with_prefix("rerun-redaction-") {
                Ok(dir) => self.scratch = Some(dir),
                Err(err) => {
                    re_log::warn_once!(
                        "Failed to create a scratch directory for analytics redaction: {err}"
                    );
                }
            }
        }

        config.mcp_servers.clear();
        config.cwd = self.scratch.as_ref()?.path().to_owned();
        config.model_preferences = REDACTION_MODELS
            .iter()
            .map(|&model| model.to_owned())
            .collect();
        Some(config)
    }
}

fn spawn_redaction_thread() -> Option<Sender<RedactionRequest>> {
    let (requests, receiver) = crossbeam::channel::bounded(REDACTION_QUEUE_CAPACITY);
    match std::thread::Builder::new()
        .name("agent-analytics-redaction".to_owned())
        .spawn(move || redaction_thread(&receiver))
    {
        Ok(_) => Some(requests),
        Err(err) => {
            re_log::warn_once!("Failed to start analytics redaction thread: {err}");
            None
        }
    }
}

fn redaction_thread(requests: &Receiver<RedactionRequest>) {
    let mut queue = VecDeque::new();
    let mut in_flight: Option<InFlight> = None;

    loop {
        let request = if in_flight.is_none() && queue.is_empty() {
            match requests.recv() {
                Ok(request) => Some(request),
                Err(_) => return,
            }
        } else {
            match requests.recv_timeout(Duration::from_millis(50)) {
                Ok(request) => Some(request),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        };
        if let Some(request) = request {
            queue.push_back(request);
        }
        queue.extend(requests.try_iter());

        if in_flight.is_none()
            && let Some(RedactionRequest { report, config }) = queue.pop_front()
        {
            let mut redactor = AgentSession::default();
            redactor.start(config, || {});
            in_flight = Some(InFlight {
                redactor,
                report,
                started: Instant::now(),
                prompt_sent: false,
            });
        }

        let finished = in_flight.as_mut().and_then(InFlight::update);
        if let Some(text) = finished
            && let Some(mut finished) = in_flight.take()
        {
            finished.redactor.stop();
            record(&finished.report, text);
        }
    }
}

impl InFlight {
    fn update(&mut self) -> Option<RedactableText> {
        self.redactor.poll_events();
        if !self.prompt_sent && self.redactor.is_ready() {
            self.prompt_sent = self.redactor.send_prompt(redaction_prompt(&self.report));
        }

        if let Some(finished) = self.redactor.take_finished_turns().pop() {
            return Some(parse_redacted(&finished.response).unwrap_or_else(|| {
                re_log::debug!("The agent did not return redacted analytics; sending without text");
                RedactableText::failed("unusable answer")
            }));
        }

        let reason = if REDACTION_TIMEOUT < self.started.elapsed() {
            Some("timed out")
        } else if !self.redactor.pending_permissions().is_empty() {
            Some("redactor asked for permission")
        } else if self.prompt_sent && !self.redactor.is_connected() {
            Some("redactor disconnected")
        } else {
            None
        };
        if let Some(reason) = reason {
            re_log::debug!("Redacting analytics failed ({reason}); sending without text");
            Some(RedactableText::failed(reason))
        } else {
            None
        }
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

/// The redaction instructions followed by the JSON to redact.
fn redaction_prompt(report: &TurnReport) -> String {
    let json =
        serde_json::to_string_pretty(&RedactableText::from_report(report)).unwrap_or_default();
    format!("{REDACTION_INSTRUCTIONS}\n\n{json}")
}

/// The JSON object in the agent's answer, tolerating text or code fences around it.
fn parse_redacted(response: &str) -> Option<RedactableText> {
    let start = response.find('{')?;
    let end = response.rfind('}')?;
    serde_json::from_str(&response[start..=end]).ok()
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
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn parses_redacted_json_with_noise_around_it() {
        let response = "Here you go:\n```json\n{\"prompt\": \"hi <name>\", \"response\": \"ok\", \
                        \"failed_tool_calls\": [], \"errors\": [\"<id> missing\"]}\n```";
        let parsed = parse_redacted(response).expect("valid");
        assert_eq!(parsed.prompt, "hi <name>");
        assert_eq!(parsed.errors, vec!["<id> missing"]);

        assert_eq!(parse_redacted("no json here"), None);
        assert_eq!(parse_redacted("{\"prompt\": 1}"), None);
    }

    #[test]
    fn the_redactor_runs_outside_the_users_project() {
        let config = LaunchConfig {
            command: PathBuf::from("agent"),
            args: Vec::new(),
            env: Vec::new(),
            cwd: std::env::current_dir().expect("cwd"),
            additional_directories: Vec::new(),
            mcp_servers: vec![re_agent_ui::McpStdioServer {
                name: "rerun".to_owned(),
                command: PathBuf::from("rerun"),
                args: vec!["viewer-mcp".to_owned()],
            }],
            log_protocol: false,
            preamble: None,
            model_preferences: Vec::new(),
        };
        let mut analytics = TurnAnalytics::default();
        let config = analytics
            .redactor_config(config)
            .expect("a scratch directory");
        assert!(config.mcp_servers.is_empty());
        assert_eq!(
            config.model_preferences.first().map(String::as_str),
            Some("sonnet")
        );
        assert_ne!(config.cwd, std::env::current_dir().expect("cwd"));
        assert!(config.cwd.is_dir());
        assert_eq!(std::fs::read_dir(&config.cwd).expect("read_dir").count(), 0);
    }

    #[test]
    fn prompt_contains_instructions_and_json() {
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
            errors: vec!["boom".to_owned()],
            permissions_requested: 0,
            permissions_rejected: 0,
        };
        let prompt = redaction_prompt(&report);
        assert!(prompt.starts_with("# Redact analytics"), "{prompt}");
        assert!(prompt.contains("\"prompt\": \"hello\""));
        assert!(prompt.contains("\"boom\""));
    }
}
