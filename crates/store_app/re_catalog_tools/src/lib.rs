//! Rerun catalog operations as tools an LLM agent can call.
//!
//! Independent of how the tools reach the agent: [`CatalogTools`] is a
//! [`re_agent_tools::AgentToolbox`], which an MCP server or an in-process agent can offer.
//!
//! The tools wrap [`re_catalog_client::CatalogClient`], the same client the Python
//! `CatalogClient` and the viewer use, and answer in JSON.

mod args;
mod catalog_tools;
mod error;
#[cfg(not(target_arch = "wasm32"))]
mod login;
mod metadata;
mod query;
mod tool;
mod write;

pub use self::catalog_tools::CatalogTools;
pub use self::error::CatalogToolError;
pub use self::tool::CatalogTool;
