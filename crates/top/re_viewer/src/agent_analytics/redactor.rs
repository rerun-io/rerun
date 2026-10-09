//! Removes sensitive data from JSON with a fresh, tool-less agent session.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crossbeam::channel::{Receiver, RecvTimeoutError, Sender, TrySendError};
use re_agent_ui::{AgentSession, LaunchConfig};
use serde_json::Value;
use tempfile::TempDir;

/// How long the redacting agent gets before the turn is sent with its text fields dropped.
const REDACTION_TIMEOUT: Duration = Duration::from_mins(2);

/// Maximum number of turns waiting for redaction before new text is dropped.
const REDACTION_QUEUE_CAPACITY: usize = 1024;

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

/// Why a redaction produced no redacted value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedactionError {
    NoScratchDirectory,
    NoThread,
    QueueFull,
    ThreadGone,
    TimedOut,
    AskedForPermission,
    Disconnected,
    UnusableAnswer,
}

impl std::fmt::Display for RedactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NoScratchDirectory => "no scratch directory",
            Self::NoThread => "no redaction thread",
            Self::QueueFull => "queue full",
            Self::ThreadGone => "redaction thread gone",
            Self::TimedOut => "timed out",
            Self::AskedForPermission => "redactor asked for permission",
            Self::Disconnected => "redactor disconnected",
            Self::UnusableAnswer => "unusable answer",
        })
    }
}

/// Called once with the redacted JSON, or with the reason there is none.
pub type OnRedacted = Box<dyn FnOnce(Result<Value, RedactionError>) + Send>;

/// JSON waiting on the redacting agent.
struct InFlight {
    redactor: AgentSession,
    json: Value,
    on_redacted: OnRedacted,
    started: Instant,
    prompt_sent: bool,
}

/// Work sent to the redaction thread.
struct RedactionRequest {
    json: Value,
    config: LaunchConfig,
    on_redacted: OnRedacted,
}

/// Redacts JSON on a background thread, where a fresh, tool-less agent session redacts each one.
/// Requests are handled one at a time, and nothing carries over between sessions.
#[derive(Default)]
pub struct Redactor {
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

impl Redactor {
    /// Queue `json` for redaction.
    ///
    /// `on_redacted` gets `json` with its sensitive data replaced, in the same shape, or the
    /// reason there is none. It runs on the redaction thread, or right away when the redaction
    /// cannot start, and never for a request that [`Self::discard_pending`] dropped.
    /// `config` is the launch config of the agent to redact with.
    pub fn redact(&mut self, json: Value, config: LaunchConfig, on_redacted: OnRedacted) {
        let Some(config) = self.redactor_config(config) else {
            on_redacted(Err(RedactionError::NoScratchDirectory));
            return;
        };
        let request = RedactionRequest {
            json,
            config,
            on_redacted,
        };
        let Some(requests) = self.requests() else {
            (request.on_redacted)(Err(RedactionError::NoThread));
            return;
        };
        if let Err(err) = requests.try_send(request) {
            let (request, reason) = match err {
                TrySendError::Full(request) => {
                    re_log::warn_once!("Analytics redaction queue is full");
                    (request, RedactionError::QueueFull)
                }
                TrySendError::Disconnected(request) => {
                    self.requests = None;
                    (request, RedactionError::ThreadGone)
                }
            };
            (request.on_redacted)(Err(reason));
        }
    }

    /// Stop redaction and drop the requests that are not done.
    ///
    /// Dropping the sender ends the thread, so this is free to call on every frame.
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
            && let Some(RedactionRequest {
                json,
                config,
                on_redacted,
            }) = queue.pop_front()
        {
            let mut redactor = AgentSession::default();
            redactor.start(config, || {});
            in_flight = Some(InFlight {
                redactor,
                json,
                on_redacted,
                started: Instant::now(),
                prompt_sent: false,
            });
        }

        let finished = in_flight.as_mut().and_then(InFlight::update);
        if let Some(answer) = finished
            && let Some(mut finished) = in_flight.take()
        {
            finished.redactor.stop();
            (finished.on_redacted)(answer);
        }
    }
}

impl InFlight {
    fn update(&mut self) -> Option<Result<Value, RedactionError>> {
        self.redactor.poll_events();
        if !self.prompt_sent && self.redactor.is_ready() {
            self.prompt_sent = self
                .redactor
                .send_prompt(redaction_prompt(&self.json).into());
        }

        if let Some(finished) = self.redactor.take_finished_turns().pop() {
            return Some(parse_redacted(&self.json, &finished.response));
        }

        let reason = if REDACTION_TIMEOUT < self.started.elapsed() {
            Some(RedactionError::TimedOut)
        } else if !self.redactor.pending_permissions().is_empty() {
            Some(RedactionError::AskedForPermission)
        } else if self.prompt_sent && !self.redactor.is_connected() {
            Some(RedactionError::Disconnected)
        } else {
            None
        };
        if let Some(reason) = reason {
            re_log::debug!("Redacting analytics failed ({reason})");
            Some(Err(reason))
        } else {
            None
        }
    }
}

/// The redaction instructions followed by the JSON to redact.
fn redaction_prompt(json: &Value) -> String {
    let json = serde_json::to_string_pretty(json).unwrap_or_default();
    format!("{REDACTION_INSTRUCTIONS}\n\n{json}")
}

/// The JSON object in the agent's answer, tolerating text or code fences around it.
///
/// An answer that is not in the shape of `original` is unusable.
fn parse_redacted(original: &Value, answer: &str) -> Result<Value, RedactionError> {
    let object = Option::zip(answer.find('{'), answer.rfind('}'))
        .and_then(|(start, end)| answer.get(start..=end));
    match object.and_then(|object| serde_json::from_str(object).ok()) {
        Some(redacted) if same_shape(original, &redacted) => Ok(redacted),
        _ => {
            re_log::debug!("The redacting agent did not return the expected JSON");
            Err(RedactionError::UnusableAnswer)
        }
    }
}

/// Whether `redacted` has the keys, array lengths, and kinds of value of `original`.
fn same_shape(original: &Value, redacted: &Value) -> bool {
    match (original, redacted) {
        (Value::Object(original), Value::Object(redacted)) => {
            original.len() == redacted.len()
                && original.iter().all(|(key, original)| {
                    redacted
                        .get(key)
                        .is_some_and(|redacted| same_shape(original, redacted))
                })
        }
        (Value::Array(original), Value::Array(redacted)) => {
            original.len() == redacted.len()
                && std::iter::zip(original, redacted)
                    .all(|(original, redacted)| same_shape(original, redacted))
        }
        (Value::String(_), Value::String(_))
        | (Value::Number(_), Value::Number(_))
        | (Value::Bool(_), Value::Bool(_))
        | (Value::Null, Value::Null) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn parses_redacted_json_with_noise_around_it() {
        let original = serde_json::json!({ "text": "hi Ada", "lines": ["at /home/ada"] });
        let answer = "Here you go:\n```json\n{\"text\": \"hi <name>\", \"lines\": [\"at /home/<name>\"]}\n```";
        assert_eq!(
            parse_redacted(&original, answer),
            Ok(serde_json::json!({ "text": "hi <name>", "lines": ["at /home/<name>"] }))
        );
        assert_eq!(
            parse_redacted(&original, "no json here"),
            Err(RedactionError::UnusableAnswer)
        );
    }

    #[test]
    fn an_answer_of_another_shape_is_unusable() {
        let original = serde_json::json!({ "text": "hi", "lines": ["a", "b"], "count": 2 });
        for answer in [
            r#"{"text": "hi", "lines": ["a", "b"]}"#,
            r#"{"text": "hi", "lines": ["a", "b"], "count": 2, "extra": 1}"#,
            r#"{"text": "hi", "lines": ["a"], "count": 2}"#,
            r#"{"text": 1, "lines": ["a", "b"], "count": 2}"#,
            r#"{"text": "hi", "lines": ["a", "b"], "count": "<id>"}"#,
        ] {
            assert_eq!(
                parse_redacted(&original, answer),
                Err(RedactionError::UnusableAnswer),
                "{answer}"
            );
        }
    }

    #[test]
    fn prompt_is_the_instructions_then_the_json() {
        let prompt = redaction_prompt(&serde_json::json!({ "text": "hello" }));
        assert!(prompt.starts_with("# Redact analytics"), "{prompt}");
        assert!(
            prompt.ends_with("\n\n{\n  \"text\": \"hello\"\n}"),
            "{prompt}"
        );
    }

    #[test]
    fn the_redactor_runs_outside_the_users_project() {
        let config = LaunchConfig {
            command: PathBuf::from("agent"),
            args: Vec::new(),
            env: Vec::new(),
            cwd: std::env::current_dir().expect("cwd"),
            additional_directories: Vec::new(),
            off_limits_directories: Vec::new(),
            mcp_servers: vec![re_agent_ui::McpStdioServer {
                name: "rerun".to_owned(),
                command: PathBuf::from("rerun"),
                args: vec!["viewer-mcp".to_owned()],
            }],
            log_protocol: false,
            preamble: None,
            model_preferences: Vec::new(),
            preferred_mode: None,
        };
        let mut redactor = Redactor::default();
        let config = redactor
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
}
