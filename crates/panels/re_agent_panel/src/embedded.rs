//! The Rerun skills from the repository, embedded by `build.rs`.

mod generated {
    include!(concat!(env!("OUT_DIR"), "/skill_files.rs"));
}

/// Every file of every skill, as `(path relative to the crate root, contents)`, e.g.
/// `("agent_context/skills/rerun-blueprint/SKILL.md", …)`.
///
/// Empty without the `agent_skills` feature.
pub use generated::SKILL_FILES;
