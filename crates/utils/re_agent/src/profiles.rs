use std::path::{Path, PathBuf};

/// Profile ids by popularity, most popular first. Decides which installed agent is picked
/// when the user has not chosen one.
const DEFAULT_AGENT_PRIORITY: [&str; 7] = [
    "claude", "codex", "gemini", "copilot", "cursor", "opencode", "goose",
];

/// How to launch a particular agent, and how to tell whether it is installed.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentProfile {
    /// Short machine-readable identifier such as `claude` or `codex`, stored in settings.
    pub id: String,

    /// Human-readable name shown in the UI, e.g. "Claude Code".
    pub name: String,

    /// One sentence for tooltips: who makes it and how it speaks ACP.
    pub description: String,

    /// Executable name or path.
    pub command: String,

    /// Arguments that put the agent in ACP mode, e.g. `--acp`.
    pub args: Vec<String>,

    /// Shown when [`Self::command`] cannot be found.
    pub install_hint: String,

    /// Terminal command that logs in to the agent, shown when it asks for auth.
    /// Empty if the agent has no separate login step.
    pub login_hint: String,

    /// Environment variables the agent needs to work well with the panel.
    pub env: Vec<(String, String)>,

    /// URL to the agent's ACP documentation or install page.
    pub url: String,
}

impl AgentProfile {
    /// Returns the resolved path of [`Self::command`], or `None` if it is not installed.
    pub fn find_executable(&self) -> Option<PathBuf> {
        find_executable(&self.command)
    }

    /// The agents we know how to launch out of the box, sorted by name.
    ///
    /// The `command` of each profile is what must be installed for it to work.
    /// The Claude Code and Codex entries go through the official ACP adapters, which need Node.js.
    pub fn builtin() -> Vec<Self> {
        builtin_profiles()
    }
}

/// A known agent plus where (and whether) it is installed on this machine.
#[derive(Clone, Debug)]
pub struct AgentEntry {
    /// How to launch this agent.
    pub profile: AgentProfile,

    /// Where the launch command was found, or `None` if the agent is not installed.
    pub executable: Option<PathBuf>,
}

impl AgentEntry {
    /// Every built-in profile, with installation checked.
    pub fn detect_all() -> Vec<Self> {
        AgentProfile::builtin()
            .into_iter()
            .map(|profile| Self {
                executable: profile.find_executable(),
                profile,
            })
            .collect()
    }

    pub fn refresh(&mut self) {
        self.executable = self.profile.find_executable();
    }

    /// The installed agent to use when the user has not picked one yet:
    /// the first installed one in a fixed popularity order, so the choice is stable
    /// from one launch to the next.
    pub fn pick_default(agents: &[Self]) -> Option<&Self> {
        let installed = agents.iter().filter(|agent| agent.executable.is_some());
        installed.min_by_key(|agent| {
            DEFAULT_AGENT_PRIORITY
                .iter()
                .position(|id| *id == agent.profile.id)
                .unwrap_or(DEFAULT_AGENT_PRIORITY.len())
        })
    }
}

fn builtin_profiles() -> Vec<AgentProfile> {
    let npx = |pkg: &str| vec!["-y".to_owned(), pkg.to_owned()];

    let mut profiles = vec![
        AgentProfile {
            id: "claude".to_owned(),
            name: "Claude Code".to_owned(),
            description: "Anthropic's Claude Code, via the official ACP adapter (needs Node.js)."
                .to_owned(),
            command: "npx".to_owned(),
            args: npx("@agentclientprotocol/claude-agent-acp"),
            install_hint: "Install Node.js (which provides npx), then run `claude` once to log in."
                .to_owned(),
            url: "https://github.com/agentclientprotocol/claude-agent-acp".to_owned(),
            login_hint: "claude  # then type /login".to_owned(),
            // Newer Claude models drop the todo tools, which is what feeds the plan view.
            env: vec![("CLAUDE_CODE_ENABLE_TODO_TOOLS".to_owned(), "1".to_owned())],
        },
        AgentProfile {
            id: "codex".to_owned(),
            name: "Codex".to_owned(),
            description: "OpenAI's Codex, via the official ACP adapter (needs Node.js).".to_owned(),
            command: "npx".to_owned(),
            args: npx("@agentclientprotocol/codex-acp"),
            install_hint: "Install Node.js (which provides npx), then run `codex` once to log in."
                .to_owned(),
            url: "https://github.com/agentclientprotocol/codex-acp".to_owned(),
            login_hint: "codex login".to_owned(),
            env: Vec::new(),
        },
        AgentProfile {
            id: "gemini".to_owned(),
            name: "Gemini CLI".to_owned(),
            description: "Google's Gemini CLI, native ACP support.".to_owned(),
            command: "gemini".to_owned(),
            args: vec!["--acp".to_owned()],
            install_hint: "npm install -g @google/gemini-cli".to_owned(),
            url: "https://geminicli.com/docs/cli/acp-mode/".to_owned(),
            login_hint: "gemini  # pick a login method when asked".to_owned(),
            env: Vec::new(),
        },
        AgentProfile {
            id: "copilot".to_owned(),
            name: "GitHub Copilot CLI".to_owned(),
            description: "GitHub Copilot CLI, native ACP support.".to_owned(),
            command: "copilot".to_owned(),
            args: vec!["--acp".to_owned()],
            install_hint: "npm install -g @github/copilot".to_owned(),
            url: "https://docs.github.com/en/copilot/reference/copilot-cli-reference/acp-server"
                .to_owned(),
            login_hint: "copilot  # then type /login".to_owned(),
            env: Vec::new(),
        },
        AgentProfile {
            id: "opencode".to_owned(),
            name: "OpenCode".to_owned(),
            description: "OpenCode, native ACP support.".to_owned(),
            command: "opencode".to_owned(),
            args: vec!["acp".to_owned()],
            install_hint: "curl -fsSL https://opencode.ai/install | bash".to_owned(),
            url: "https://opencode.ai/docs/acp/".to_owned(),
            login_hint: "opencode auth login".to_owned(),
            env: Vec::new(),
        },
        AgentProfile {
            id: "goose".to_owned(),
            name: "Goose".to_owned(),
            description: "Block's Goose, native ACP support.".to_owned(),
            command: "goose".to_owned(),
            args: vec!["acp".to_owned()],
            install_hint: "Download from https://github.com/block/goose/releases".to_owned(),
            url: "https://github.com/block/goose".to_owned(),
            login_hint: "goose configure".to_owned(),
            env: Vec::new(),
        },
        AgentProfile {
            id: "cursor".to_owned(),
            name: "Cursor CLI".to_owned(),
            description: "Cursor's CLI agent, native ACP support.".to_owned(),
            command: "cursor-agent".to_owned(),
            args: vec!["acp".to_owned()],
            install_hint: "curl https://cursor.com/install -fsS | bash".to_owned(),
            url: "https://cursor.com/docs/cli/acp".to_owned(),
            login_hint: "cursor-agent login".to_owned(),
            env: Vec::new(),
        },
    ];
    profiles.sort_by_key(|profile| profile.name.to_lowercase());
    profiles
}

/// Looks for an executable on `PATH`, plus a few directories that installers use
/// but that are often missing from the `PATH` of a GUI app.
pub fn find_executable(command: &str) -> Option<PathBuf> {
    let command = Path::new(command);

    if command.components().count() > 1 {
        return is_executable(command).then(|| command.to_path_buf());
    }

    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();

    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".cargo/bin"));
        dirs.push(home.join(".opencode/bin"));
        dirs.push(home.join(".npm-global/bin"));
        dirs.push(home.join(".bun/bin"));
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));

    let candidates: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };

    dirs.into_iter()
        .filter(|dir| !dir.as_os_str().is_empty())
        .flat_map(|dir| {
            candidates.iter().map(move |ext| {
                let mut file_name = command.as_os_str().to_owned();
                file_name.push(ext);
                dir.join(file_name)
            })
        })
        .find(|path| is_executable(path))
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    cfg_select! {
        unix => {
            use std::os::unix::fs::PermissionsExt as _;
            metadata.permissions().mode() & 0o111 != 0
        }
        _ => true,
    }
}
