//! What sessions report to their host.

use crate::{McpStartupFailure, StartupReport, TurnReport};

/// Reports from one or more sessions, until the host takes them.
#[derive(Default)]
pub struct SessionReports {
    /// Turns that finished, in the order they finished.
    pub turns: Vec<TurnReport>,

    /// Starts of agents that ended, in the order they ended. A session starts its agent at most
    /// once.
    pub startups: Vec<StartupReport>,

    /// MCP servers that failed to start, including the ones the agent replaced with a definition
    /// from its own config.
    pub mcp_startup_failures: Vec<McpStartupReport>,
}

/// An MCP server that failed to start, and which agent reported it.
#[derive(Clone, Debug)]
pub struct McpStartupReport {
    /// The agent's self-reported name, if it had introduced itself by then.
    pub agent: Option<String>,

    /// What failed.
    pub failure: McpStartupFailure,
}

impl SessionReports {
    /// Moves the reports of `other` to the end of these.
    pub fn append(&mut self, other: Self) {
        let Self {
            turns,
            startups,
            mcp_startup_failures,
        } = other;
        self.turns.extend(turns);
        self.startups.extend(startups);
        self.mcp_startup_failures.extend(mcp_startup_failures);
    }
}
