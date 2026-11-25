//! Shortening of tool-call titles for display, so that the command or file stays visible when
//! the row is truncated.

use std::path::{Path, PathBuf};

use re_agent::AgentSession;

/// `call_title` as shown in a tool-call row of `session`.
///
/// See [`shorten_title`]. There is no home directory on the web.
pub fn short_tool_title(session: &AgentSession, call_title: &str) -> String {
    let home: Option<PathBuf> = cfg_select! {
        target_arch = "wasm32" => None,
        _ => std::env::home_dir(),
    };
    shorten_title(
        session.cwd(),
        session.readable_directories(),
        home.as_deref(),
        call_title,
    )
}

/// Shorten the paths in a tool-call title, and drop a leading `cd` into the working directory.
///
/// Paths inside `cwd` or one of `readable_directories` become relative to the innermost of them,
/// and other paths inside `home` start with `~`.
/// Directories only match whole path components, so `/home/al` does not match `/home/alice`.
/// Only for display: the result may not be a valid command.
fn shorten_title(
    cwd: &Path,
    readable_directories: &[PathBuf],
    home: Option<&Path>,
    title: &str,
) -> String {
    let dir_str = |dir: &Path| {
        let dir = dir.to_str()?.trim_end_matches(is_separator);
        // A root directory would match every absolute path.
        (1 < dir.len()).then(|| dir.to_owned())
    };

    let mut pairs: Vec<(String, &str)> = Vec::new();
    let relative_dirs = std::iter::chain(
        std::iter::once(cwd),
        readable_directories.iter().map(PathBuf::as_path),
    );
    for dir in relative_dirs {
        if let Some(dir) = dir_str(dir) {
            pairs.push((format!("{dir}/"), ""));
            pairs.push((format!("{dir}\\"), ""));
            pairs.push((dir, "."));
        }
    }
    if let Some(home) = home.and_then(dir_str) {
        pairs.push((home, "~"));
    }
    pairs.sort_by_key(|(dir, _)| std::cmp::Reverse(dir.len()));

    let mut title = title.to_owned();
    for (dir, replacement) in &pairs {
        replace_directory(&mut title, dir, replacement);
    }
    strip_cd_into_cwd(&title).unwrap_or(&title).to_owned()
}

/// Replace each occurrence of `dir` that is a whole path component sequence.
///
/// `dir` must not follow a path character. A `dir` ending in a separator must be followed by a
/// path character, and any other `dir` must not be followed by one, unless it is a separator.
/// Occurrences inside a URL are kept.
fn replace_directory(title: &mut String, dir: &str, replacement: &str) {
    let mut start = 0;
    while let Some(pos) = title[start..].find(dir).map(|pos| start + pos) {
        let end = pos + dir.len();
        let before = title[..pos].chars().next_back();
        let after = title[end..].chars().next();
        let starts_component = before.is_none_or(|c| !is_path_char(c));
        let ends_component = if dir.ends_with(is_separator) {
            after.is_some_and(is_path_char)
        } else {
            after.is_none_or(|c| !is_path_char(c) || is_separator(c))
        };
        let in_url = title[..pos]
            .rsplit(char::is_whitespace)
            .next()
            .is_some_and(|token| token.contains("://"));
        if starts_component && ends_component && !in_url {
            title.replace_range(pos..end, replacement);
            start = pos + replacement.len();
        } else {
            start = end;
        }
    }
}

/// What follows `cd . && ` or `cd .; ` at the start of `command`, if it starts with either.
fn strip_cd_into_cwd(command: &str) -> Option<&str> {
    let rest = command.strip_prefix("cd ")?.trim_start();
    let rest = ["\".\"", "'.'", "./", "."]
        .iter()
        .find_map(|dot| rest.strip_prefix(dot))?
        .trim_start();
    let rest = rest.strip_prefix("&&").or_else(|| rest.strip_prefix(';'))?;
    Some(rest.trim_start()).filter(|rest| !rest.is_empty())
}

fn is_separator(c: char) -> bool {
    c == '/' || c == '\\'
}

fn is_path_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            '#' | '%' | '+' | '-' | '.' | '/' | '@' | '\\' | '_' | '~'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CWD: &str = "/Users/alice/Library/Caches/io.rerun.Rerun/agent-cwd";
    const AGENT_DIR: &str = "/Users/alice/Library/Caches/io.rerun.Rerun/agent";
    const HOME: &str = "/Users/alice";

    fn shorten(title: &str) -> String {
        shorten_title(
            Path::new(CWD),
            &[PathBuf::from(AGENT_DIR)],
            Some(Path::new(HOME)),
            title,
        )
    }

    #[test]
    fn drops_cd_into_cwd() {
        assert_eq!(shorten(&format!("cd {CWD} && uv run x.py")), "uv run x.py");
        assert_eq!(shorten(&format!("cd {CWD}/ && uv run x.py")), "uv run x.py");
        assert_eq!(shorten(&format!("cd {CWD:?} && ls")), "ls");
        assert_eq!(shorten(&format!("cd {CWD}; ls")), "ls");
        assert_eq!(shorten(&format!("cd {CWD}")), "cd .");
        assert_eq!(shorten(&format!("cd {CWD} &&")), "cd . &&");
        assert_eq!(shorten(&format!("cd {CWD}/sub && ls")), "cd sub && ls");
        assert_eq!(shorten(&format!("cd {CWD} || ls")), "cd . || ls");
    }

    #[test]
    fn relative_to_cwd() {
        assert_eq!(
            shorten(&format!("Read {CWD}/scripts/foo.py")),
            "Read scripts/foo.py"
        );
        assert_eq!(shorten(&format!("ls {CWD}")), "ls .");
        assert_eq!(shorten(&format!("ls {CWD}/")), "ls ./");
        assert_eq!(shorten(&format!("--dir={CWD}/x")), "--dir=x");
    }

    #[test]
    fn relative_to_readable_directory() {
        assert_eq!(
            shorten(&format!(
                "Read {AGENT_DIR}/agent_context/skills/rerun-x/SKILL.md"
            )),
            "Read agent_context/skills/rerun-x/SKILL.md"
        );
    }

    #[test]
    fn home_becomes_tilde() {
        assert_eq!(
            shorten("cd /Users/alice/dev/x && cargo clippy"),
            "cd ~/dev/x && cargo clippy"
        );
        assert_eq!(shorten("ls /Users/alice"), "ls ~");
        assert_eq!(shorten("ls /Users/alice/"), "ls ~/");
    }

    #[test]
    fn only_whole_components() {
        assert_eq!(shorten("ls /Users/alicex/y"), "ls /Users/alicex/y");
        assert_eq!(
            shorten(&format!("ls {CWD}2/y")),
            "ls ~/Library/Caches/io.rerun.Rerun/agent-cwd2/y"
        );
        assert_eq!(shorten("ls /mnt/Users/alice/y"), "ls /mnt/Users/alice/y");
        for c in ['#', '%', '+', '@'] {
            let title = format!("ls {HOME}{c}server");
            assert_eq!(shorten(&title), title);
        }
        assert_eq!(
            shorten(&format!("Read {CWD}/@types/node")),
            "Read @types/node"
        );
    }

    #[test]
    fn urls_are_kept() {
        let title = format!("curl https://example.com/?path={CWD}/file");
        assert_eq!(shorten(&title), title);
        let title = format!("curl \"https://example.com/?dir={HOME}\"");
        assert_eq!(shorten(&title), title);
        assert_eq!(
            shorten(&format!("curl https://example.com -o {CWD}/file")),
            "curl https://example.com -o file"
        );
    }

    #[test]
    fn without_home_or_readable_directories() {
        let short = shorten_title(Path::new("/w"), &[], None, "cat /w/a /Users/alice/b");
        assert_eq!(short, "cat a /Users/alice/b");
    }

    #[test]
    fn root_and_trailing_slashes() {
        assert_eq!(
            shorten_title(Path::new("/"), &[], None, "ls /etc"),
            "ls /etc"
        );
        assert_eq!(
            shorten_title(Path::new("/w/"), &[], None, "ls /w/a"),
            "ls a"
        );
    }

    #[test]
    fn windows_paths() {
        let short = shorten_title(
            Path::new(r"C:\work"),
            &[],
            Some(Path::new(r"C:\Users\alice")),
            r"cd C:\work && type C:\Users\alice\notes.txt",
        );
        assert_eq!(short, r"type ~\notes.txt");
    }
}
