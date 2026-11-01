//! Agent harnesses: coding agents such as Claude Code or Codex, installed on the user's machine.

#[cfg(agent_harness_backend)]
mod backend;
mod config;
#[cfg(agent_harness_backend)]
mod launch;
mod profiles;

#[cfg(all(agent_harness_backend, test))]
pub use backend::forward_answer;
#[cfg(agent_harness_backend)]
pub use backend::spawn;
pub use config::{AgentHarnessConfig, McpStdioServer};
#[cfg(agent_harness_backend)]
pub use profiles::find_executable;
pub use profiles::{AgentEntry, AgentKind, AgentProfile, AgentProfileId};
