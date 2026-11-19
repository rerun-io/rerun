//! Reports describing completed agent turns.

use std::path::PathBuf;
use std::time::Duration;

use web_time::Instant;

use agent_client_protocol_schema::v1::{ContentBlock, ToolCallContent};

use crate::transcript::ToolCallState;

/// Bookkeeping for the turn in progress, turned into a [`TurnReport`] when it ends.
pub struct TurnStart {
    pub prompt: String,
    pub started: Instant,

    /// Index of the first transcript item belonging to this turn.
    pub transcript_start: usize,
    pub permissions_requested: u32,
    pub permissions_rejected: u32,
    waiting_since: Option<Instant>,
    waited: Duration,
}

impl TurnStart {
    pub fn new(prompt: String, transcript_start: usize) -> Self {
        Self {
            prompt,
            started: Instant::now(),
            transcript_start,
            permissions_requested: 0,
            permissions_rejected: 0,
            waiting_since: None,
            waited: Duration::ZERO,
        }
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn testing(transcript_start: usize) -> Self {
        Self::new(String::new(), transcript_start)
    }

    pub fn set_waiting(&mut self, waiting: bool, now: Instant) {
        match (waiting, self.waiting_since) {
            (true, None) => self.waiting_since = Some(now),
            (false, Some(since)) => {
                self.waited += now.saturating_duration_since(since);
                self.waiting_since = None;
            }
            _ => {}
        }
    }

    /// Time since the turn started, minus the time spent waiting on the user, as of `now`.
    pub fn working_time(&self, now: Instant) -> Duration {
        let waiting_now = self
            .waiting_since
            .map_or(Duration::ZERO, |since| now.saturating_duration_since(since));
        now.saturating_duration_since(self.started)
            .saturating_sub(self.waited + waiting_now)
    }
}

/// How a turn ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnOutcome {
    /// The agent finished answering.
    Completed,

    /// The user cancelled the turn.
    Cancelled,

    /// The agent hit its token or tool-call budget.
    Truncated,

    /// The agent declined to continue.
    Refused,

    /// The agent reported an error; the message is among the report's errors.
    Error,

    /// The agent was stopped or exited before finishing.
    Aborted,
}

impl TurnOutcome {
    /// A short `snake_case` name for logs and analytics.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Truncated => "truncated",
            Self::Refused => "refused",
            Self::Error => "error",
            Self::Aborted => "aborted",
        }
    }
}

/// What happened during one prompt, for hosts that want to log or record it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnReport {
    /// The agent's self-reported name, e.g. "Claude Code", if it had introduced itself by then.
    pub agent: Option<String>,

    /// The model the agent used, e.g. "Opus 5.5".
    pub model: Option<String>,

    /// What the user typed, without the preamble.
    pub prompt: String,

    /// From sending the prompt to the agent finishing.
    pub duration: Duration,

    pub outcome: TurnOutcome,

    /// The agent's final answer: the text of its last message in the turn.
    pub response: String,

    /// Tool calls the agent made during the turn.
    pub tool_calls: u32,

    /// Context-window usage the agent last reported, in tokens.
    pub tokens_used: Option<u64>,
    pub token_limit: Option<u64>,

    /// Tool calls that failed, as `title: first line of the output`.
    pub failed_tool_calls: Vec<String>,

    /// Paths the agent touched inside the directories it was asked to stay out of.
    ///
    /// Any touch counts, not only a read: a path an off-limits tool call executed in or wrote to
    /// is listed the same way. The request is made in the preamble and nothing enforces it, so
    /// this is how the host finds out whether it was honored. Empty whenever no such directory
    /// was declared.
    pub off_limits_paths: Vec<PathBuf>,

    /// Errors reported in the transcript during the turn.
    pub errors: Vec<String>,

    /// Permission requests the user was asked about, and how many they rejected.
    pub permissions_requested: u32,
    pub permissions_rejected: u32,
}

/// `title: first line of the output`, for a failed tool call.
pub fn describe_failed_tool_call(call: &ToolCallState) -> String {
    let output = call
        .content
        .iter()
        .find_map(|content| match content {
            ToolCallContent::Content(content) => match &content.content {
                ContentBlock::Text(text) => text.text.lines().next(),
                _ => None,
            },
            _ => None,
        })
        .or_else(|| call.raw_output.as_ref()?.as_str()?.lines().next());
    match output {
        Some(output) => format!("{}: {output}", call.title),
        None => call.title.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waiting_on_the_user_is_not_working_time() {
        let mut turn = TurnStart::testing(1);
        let start = turn.started;
        let secs = |n| start + Duration::from_secs(n);

        turn.set_waiting(true, secs(10));
        turn.set_waiting(true, secs(20));
        turn.set_waiting(false, secs(70));
        assert_eq!(turn.working_time(secs(100)), Duration::from_secs(40));

        turn.set_waiting(true, secs(100));
        assert_eq!(turn.working_time(secs(130)), Duration::from_secs(40));
    }
}
