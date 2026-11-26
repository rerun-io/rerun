//! [`SkillsToolbox`]: the Rerun skills, for a model-API agent to read.
//!
//! A model-API agent has no filesystem, so it reads the embedded skills through this tool.
//! An agent harness (Claude Code, Codex, …) instead reads the same files from disk, where the
//! panel unpacks them, and does not get this tool:
//! - Some skills run scripts that ship next to their `SKILL.md`, e.g. `uv run scripts/….py`.
//!   Those must exist as files for the agent to run them.
//! - A harness reads, greps, and loads skills from disk natively, which works better than a tool
//!   it has to be told about.
//! - The cost is two delivery paths for one set of files, both fed by `SKILL_FILES`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use futures::future::BoxFuture;
use re_agent_tools::{AgentToolbox, ToolDefinition, ToolEffect, ToolError, ToolOutput};
use serde_json::{Map, Value, json};

/// The one tool of [`SkillsToolbox`].
pub const SKILL_READ: &str = "skill_read";

/// Where the skills are among the embedded paths.
const SKILLS_PREFIX: &str = "agent_context/skills/";

/// The file of a skill that is read unless another is asked for.
const SKILL_MD: &str = "SKILL.md";

#[derive(Debug, thiserror::Error)]
enum SkillError {
    #[error("Unknown tool `{name}`")]
    UnknownTool { name: String },

    #[error("Missing the `skill` argument")]
    MissingSkill,

    #[error("The `{argument}` argument must be a string")]
    NotAString { argument: &'static str },

    #[error("There is no skill `{skill}`. The skills are: {available}")]
    UnknownSkill { skill: String, available: String },

    #[error("The skill `{skill}` has no file `{file}`. Its files are: {available}")]
    UnknownFile {
        skill: String,
        file: String,
        available: String,
    },

    #[error("The file `{file}` of the skill `{skill}` is not text")]
    NotText {
        skill: String,
        file: String,
        source: std::str::Utf8Error,
    },
}

/// The files of each skill: skill name → path within the skill directory → contents.
type SkillFiles = BTreeMap<&'static str, BTreeMap<&'static str, &'static [u8]>>;

/// Serves the files of the Rerun skills through [`SKILL_READ`].
pub struct SkillsToolbox {
    skills: SkillFiles,
    definition: ToolDefinition,
}

impl SkillsToolbox {
    /// The skills embedded in this binary, or `None` if there are none.
    pub fn embedded() -> Option<Self> {
        Self::new(crate::embedded::SKILL_FILES)
    }

    /// The skills among `files`, given as `(path relative to the crate root, contents)`, or
    /// `None` if there are none.
    fn new(files: &'static [(&'static str, &'static [u8])]) -> Option<Self> {
        let mut skills = SkillFiles::new();
        for (path, contents) in files {
            let Some((skill, file)) = path
                .strip_prefix(SKILLS_PREFIX)
                .and_then(|path| path.split_once('/'))
            else {
                continue;
            };
            skills.entry(skill).or_default().insert(file, *contents);
        }
        skills.retain(|_, files| files.contains_key(SKILL_MD));
        if skills.is_empty() {
            return None;
        }

        let definition = definition(&skills);
        Some(Self { skills, definition })
    }

    fn read(&self, arguments: &Map<String, Value>) -> Result<String, SkillError> {
        let string_argument = |argument: &'static str| {
            arguments
                .get(argument)
                .map(|value| value.as_str().ok_or(SkillError::NotAString { argument }))
                .transpose()
        };
        let skill = string_argument("skill")?.ok_or(SkillError::MissingSkill)?;
        let file = string_argument("file")?.unwrap_or(SKILL_MD);

        let files = self
            .skills
            .get(skill)
            .ok_or_else(|| SkillError::UnknownSkill {
                skill: skill.to_owned(),
                available: quoted_list(self.skills.keys()),
            })?;
        let contents = files.get(file).ok_or_else(|| SkillError::UnknownFile {
            skill: skill.to_owned(),
            file: file.to_owned(),
            available: quoted_list(files.keys()),
        })?;
        let mut text = std::str::from_utf8(contents)
            .map_err(|source| SkillError::NotText {
                skill: skill.to_owned(),
                file: file.to_owned(),
                source,
            })?
            .to_owned();

        let other_files: Vec<_> = files.keys().filter(|&&other| other != file).collect();
        if file == SKILL_MD && !other_files.is_empty() {
            writeln!(
                text,
                "\n\n---\nOther files of this skill, to read with `{SKILL_READ}` and `file`: {}",
                quoted_list(other_files)
            )
            .ok();
        }
        Ok(text)
    }
}

impl AgentToolbox for SkillsToolbox {
    fn definitions(&self) -> Vec<ToolDefinition> {
        vec![self.definition.clone()]
    }

    fn requires_approval(&self, _name: &str, _arguments: &Map<String, Value>) -> bool {
        self.definition.effect.requires_approval()
    }

    fn call(
        &self,
        name: &str,
        arguments: Map<String, Value>,
    ) -> BoxFuture<'static, Result<Vec<ToolOutput>, ToolError>> {
        let result = if name == SKILL_READ {
            self.read(&arguments)
        } else {
            Err(SkillError::UnknownTool {
                name: name.to_owned(),
            })
        };
        Box::pin(std::future::ready(
            result
                .map(|text| vec![ToolOutput::Text(text)])
                .map_err(Into::into),
        ))
    }
}

/// The [`SKILL_READ`] tool, listing every skill with its description, so that the model knows
/// when to read one without a round trip.
fn definition(skills: &SkillFiles) -> ToolDefinition {
    let mut description = format!(
        "Read a Rerun skill: instructions for a kind of task, written for agents. \
         Before starting a task that a skill below covers, read its `{SKILL_MD}` with this tool \
         and follow it. A skill may point at more of its files: read those by passing `file`.\n\
         Skills are also written for agents with a shell and a filesystem. \
         Where one says to run a command or a script, give the user the code to run instead.\n\n\
         Skills:\n"
    );
    for (name, files) in skills {
        let skill_md = files
            .get(SKILL_MD)
            .and_then(|contents| std::str::from_utf8(contents).ok())
            .unwrap_or_default();
        if let Some(skill_description) = frontmatter_description(skill_md) {
            writeln!(description, "- `{name}`: {skill_description}").ok();
        } else {
            re_log::warn_once!("The Rerun skill `{name}` has no description in its frontmatter");
            writeln!(description, "- `{name}`").ok();
        }
    }

    let names: Vec<_> = skills.keys().collect();
    let input_schema = json!({
        "type": "object",
        "properties": {
            "skill": {
                "type": "string",
                "description": "The name of the skill, e.g. `rerun-blueprint`.",
                "enum": names,
            },
            "file": {
                "type": "string",
                "description": format!(
                    "A file of the skill, relative to its directory, \
                     e.g. `references/registering.md`. Defaults to `{SKILL_MD}`."
                ),
            },
        },
        "required": ["skill"],
        "additionalProperties": false,
    });

    ToolDefinition {
        name: SKILL_READ.to_owned(),
        description,
        input_schema: match input_schema {
            Value::Object(schema) => schema,
            _ => Map::new(),
        },
        effect: ToolEffect::ReadOnly,
        // The skills are bundled with the viewer.
        open_world: false,
    }
}

/// The `description` in the YAML frontmatter of a `SKILL.md`.
///
/// Handles the forms our skills use: a plain or quoted scalar on one line, and a folded (`>`)
/// or literal (`|`) block on the indented lines after it.
fn frontmatter_description(skill_md: &str) -> Option<String> {
    let mut lines = skill_md.lines();
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    let mut frontmatter = lines.take_while(|line| line.trim_end() != "---");
    let value = frontmatter.find_map(|line| line.strip_prefix("description:"))?;
    let value = value.trim();

    if value.starts_with('>') || value.starts_with('|') {
        let block: Vec<&str> = frontmatter
            .take_while(|line| line.starts_with(' ') || line.trim().is_empty())
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        return Some(block.join(" "));
    }

    let unquoted = if let Some(inner) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        inner.replace("\\\"", "\"").replace("\\\\", "\\")
    } else if let Some(inner) = value
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
    {
        inner.replace("''", "'")
    } else {
        value.to_owned()
    };
    Some(unquoted)
}

fn quoted_list<'a>(items: impl IntoIterator<Item = &'a &'a str>) -> String {
    items
        .into_iter()
        .map(|item| format!("`{item}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: &[(&str, &[u8])] = &[
        ("agent_context/skills/README.md", b"# Skills"),
        (
            "agent_context/skills/alpha/SKILL.md",
            b"---\nname: alpha\ndescription: >\n  Folded\n  text.\nother: x\n---\nAlpha body",
        ),
        ("agent_context/skills/alpha/references/more.md", b"More"),
        ("agent_context/skills/alpha/blob.bin", b"\xff\xfe"),
        (
            "agent_context/skills/beta/SKILL.md",
            b"---\ndescription: \"Say \\\"hi\\\"\"\n---\nBeta body",
        ),
        ("agent_context/skills/no-skill-md/notes.md", b"Notes"),
        ("agent_context/docs/index.md", b"Docs"),
    ];

    fn read(toolbox: &SkillsToolbox, arguments: Value) -> Result<String, SkillError> {
        let Value::Object(arguments) = arguments else {
            panic!("arguments must be an object");
        };
        toolbox.read(&arguments)
    }

    #[test]
    fn frontmatter_descriptions() {
        assert_eq!(
            frontmatter_description("---\ndescription: Plain: with colon\n---\n").as_deref(),
            Some("Plain: with colon")
        );
        assert_eq!(
            frontmatter_description("---\ndescription: 'It''s'\n---\n").as_deref(),
            Some("It's")
        );
        assert_eq!(
            frontmatter_description(
                "---\r\ndescription: >\r\n  a\r\n\r\n  b\r\nname: x\r\n---\r\n"
            )
            .as_deref(),
            Some("a b")
        );
        // Only the frontmatter counts.
        assert_eq!(
            frontmatter_description("---\nname: x\n---\ndescription: body\n"),
            None
        );
        assert_eq!(
            frontmatter_description("description: no frontmatter\n"),
            None
        );
    }

    #[test]
    fn lists_only_skills_with_a_skill_md() {
        let toolbox = SkillsToolbox::new(FILES).expect("skills");
        assert_eq!(
            toolbox.skills.keys().copied().collect::<Vec<_>>(),
            ["alpha", "beta"]
        );
        let ToolDefinition { description, .. } = &toolbox.definition;
        assert!(description.contains("- `alpha`: Folded text.\n"));
        assert!(description.contains("- `beta`: Say \"hi\"\n"));

        assert!(SkillsToolbox::new(&[]).is_none());
        assert!(SkillsToolbox::new(&[("agent_context/skills/x/notes.md", b"")]).is_none());
    }

    #[test]
    fn reads_skill_files() {
        let toolbox = SkillsToolbox::new(FILES).expect("skills");

        let alpha = read(&toolbox, json!({ "skill": "alpha" })).expect("SKILL.md");
        assert!(alpha.starts_with("---\nname: alpha"));
        assert!(alpha.ends_with("`blob.bin`, `references/more.md`\n"));

        // No list of other files when there are none, or when reading another file.
        assert_eq!(
            read(&toolbox, json!({ "skill": "beta" })).expect("SKILL.md"),
            "---\ndescription: \"Say \\\"hi\\\"\"\n---\nBeta body"
        );
        assert_eq!(
            read(
                &toolbox,
                json!({ "skill": "alpha", "file": "references/more.md" })
            )
            .expect("reference"),
            "More"
        );

        assert!(matches!(
            read(&toolbox, json!({ "skill": "alpha", "file": "blob.bin" })),
            Err(SkillError::NotText { .. })
        ));
        assert!(matches!(
            read(&toolbox, json!({})),
            Err(SkillError::MissingSkill)
        ));
        assert!(matches!(
            read(&toolbox, json!({ "skill": "alpha", "file": 3 })),
            Err(SkillError::NotAString { argument: "file" })
        ));
        let err = read(&toolbox, json!({ "skill": "no-skill-md" })).expect_err("no SKILL.md");
        assert_eq!(
            err.to_string(),
            "There is no skill `no-skill-md`. The skills are: `alpha`, `beta`"
        );
        let err = read(
            &toolbox,
            json!({ "skill": "alpha", "file": "../beta/SKILL.md" }),
        )
        .expect_err("outside the skill");
        assert!(matches!(err, SkillError::UnknownFile { .. }));
    }

    #[test]
    #[cfg(feature = "agent_skills")]
    fn every_embedded_skill_has_a_description() {
        let toolbox = SkillsToolbox::embedded().expect("embedded skills");
        assert!(toolbox.skills.contains_key("rerun-blueprint"));
        for (name, files) in &toolbox.skills {
            let skill_md = std::str::from_utf8(files[SKILL_MD]).expect("UTF-8");
            let description = frontmatter_description(skill_md);
            assert!(
                description
                    .as_ref()
                    .is_some_and(|description| 20 < description.len()),
                "{name}: {description:?}"
            );
        }
    }
}
