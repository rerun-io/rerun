//! Minimal ROS 2 `.msg` reflection parser (messages only).
//!
//! This module parses the textual ROS 2 message definition format (aka `.msg`)
//! into a typed, reflection-friendly representation. It is intentionally kept
//! generic and does not rely on any pre-baked message definitions, so it can be
//! used to parse unknown types and still extract semantic meaning (types,
//! arrays, names, constants, default values).
use anyhow::Context as _;

use crate::message_spec::MessageSpecification;

pub mod deserialize;
pub mod message_spec;

/// Parse a schema name from a line starting with "MSG: ".
fn parse_schema_name(line: &str) -> Option<&str> {
    line.trim().strip_prefix("MSG: ").map(str::trim)
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageSchema {
    /// Specification of the main message type.
    pub spec: MessageSpecification,

    /// Dependent message types referenced by the main type.
    pub dependencies: Vec<MessageSpecification>, // Other message types referenced by this one.
}

impl MessageSchema {
    pub fn parse(name: &str, input: &str) -> anyhow::Result<Self> {
        let main_spec_content = extract_main_msg_spec(input);
        let specs = extract_msg_specs(input);

        let main_spec = MessageSpecification::parse(name, &main_spec_content)
            .with_context(|| format!("failed to parse main message spec `{name}`"))?;

        let mut dependencies = Vec::new();
        for (dep_name, dep_content) in specs {
            let dep_spec = MessageSpecification::parse(&dep_name, &dep_content)
                .with_context(|| format!("failed to parse dependent message spec `{dep_name}`"))?;
            dependencies.push(dep_spec);
        }

        Ok(Self {
            spec: main_spec,
            dependencies,
        })
    }
}

/// Check if a line is a schema separator (a line of at least 3 '=' characters).
pub fn is_schema_separator(line: &str) -> bool {
    let line = line.trim();
    line.len() >= 3 && line.chars().all(|c| c == '=')
}

/// Extract the main message specification from input, stopping at the first schema separator.
///
/// The main spec is everything before the first "====" separator line.
fn extract_main_msg_spec(input: &str) -> String {
    input
        .lines()
        .take_while(|line| !is_schema_separator(line))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Find "MSG: `<name>`" and take the rest as content
/// Extract all message specifications from input that are separated by schema separators.
///
/// Returns a vector of `(message_name, message_body)` pairs for each schema found.
fn extract_msg_specs(input: &str) -> Vec<(String, String)> {
    let mut specs = Vec::new();
    let mut current_section = Vec::new();

    for line in input.lines() {
        if is_schema_separator(line) {
            if let Some(spec) = parse_section(&current_section) {
                specs.push(spec);
            }
            current_section.clear();
        } else {
            current_section.push(line);
        }
    }

    // Handle the final section if it doesn't end with a separator
    if let Some(spec) = parse_section(&current_section) {
        specs.push(spec);
    }

    specs
}

/// Parse a section of lines into a (name, body) pair.
///
/// The first line should contain "MSG: `<name>`" and subsequent lines form the message body.
fn parse_section(lines: &[&str]) -> Option<(String, String)> {
    if lines.len() < 2 {
        return None;
    }

    let first_line = lines[0].trim();
    let name = parse_schema_name(first_line)?;
    let body = lines[1..].join("\n");

    Some((name.to_owned(), body))
}
