//! `relint`: Rerun's custom linter for Rust code.
//!
//! Unlike `scripts/lint.py`, which works on lines of text, `relint` parses Rust into a syntax tree
//! with [`ra_ap_syntax`] (the parser of rust-analyzer), so rules can tell code from comments and strings,
//! and see constructs that span several lines.
//!
//! With no arguments, it lints every `.rs` and `.md` file under the current directory that git knows
//! about (tracked, or untracked but not ignored). In `.md` files and in doc comments,
//! it lints the Rust code blocks.
//! Each file is linted with the `relint.toml` in its nearest ancestor directory, if any.

mod code_blocks;
mod config;
mod diagnostic;
mod format_args;
mod rules;
mod source;
mod suppression;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    process::ExitCode,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{config::Config, source::SourceFile};

/// The extensions of the files we lint: Rust code, and the Rust code blocks in markdown.
const EXTENSIONS: &[&str] = &["md", "rs"];

const USAGE: &str = "\
Usage: relint [--fix] [--list-rules] [PATH …]

Lints the given `.rs` and `.md` files, and those under the given directories that git knows about.
With no paths, lints all of them under the current directory.
In `.md` files and doc comments, the Rust code blocks are linted.
With `--fix`, applies the automatic fixes some rules have, and only reports the rest.

Allow a rule on one line with a `// relint: allow(rule_name): reason` comment,
at the end of that line, or on its own line above it.";

fn main() -> ExitCode {
    let mut paths = Vec::new();
    let mut fix = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--fix" => fix = true,
            "--list-rules" => {
                for rule in rules::RULES {
                    println!("{}", rule.name);
                }
                return ExitCode::SUCCESS;
            }
            _ if arg.starts_with('-') => {
                eprintln!("Unknown option {arg:?}\n\n{USAGE}");
                return ExitCode::from(2);
            }
            _ => paths.push(arg),
        }
    }

    match run(&paths, fix) {
        Ok(0) => {
            println!("relint finished without error");
            ExitCode::SUCCESS
        }
        Ok(num_errors) => {
            println!("relint found {num_errors} error(s)");
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("relint: {err}");
            ExitCode::from(2)
        }
    }
}

/// Why `relint` could not finish.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("No such file or directory: {path}")]
    NoSuchPath { path: String },

    #[error("Failed to run `git {command}`: {source}")]
    GitSpawn {
        command: &'static str,
        source: std::io::Error,
    },

    #[error("`git {command}` failed: {stderr}")]
    GitFailed {
        command: &'static str,
        stderr: String,
    },

    #[error("Failed to open stdin of `git check-attr`")]
    GitStdin,

    #[error("Failed to write to `git check-attr`: {0}")]
    GitWrite(#[source] std::io::Error),

    #[error("Failed to spawn thread: {0}")]
    SpawnThread(#[source] std::io::Error),

    #[error("Failed to read the current directory: {0}")]
    CurrentDir(#[source] std::io::Error),

    #[error(transparent)]
    Config(#[from] config::LoadError),

    #[error("Unknown rule {rule:?} in {}", config.display())]
    UnknownRule { rule: String, config: PathBuf },

    #[error("Failed to read file: {source}\nFile path: {path}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },

    #[error("Failed to write fixes: {source}\nFile path: {path}")]
    WriteFixes {
        path: String,
        source: std::io::Error,
    },
}

fn run(paths: &[String], fix: bool) -> Result<usize, Error> {
    let files = list_files(paths)?;
    let generated = generated_files(&files)?;
    let files: Vec<String> = files
        .into_iter()
        .filter(|path| !generated.contains(path))
        .collect();
    let configs = configs_for(&files)?;

    let outputs = lint_files_in_parallel(&files, &configs, fix);

    let mut num_errors = 0;
    for output in outputs {
        let output = output?;
        num_errors += output.len();
        for line in output {
            println!("{line}");
        }
    }
    Ok(num_errors)
}

/// Files with [`EXTENSIONS`] that git knows about, relative to the current directory.
///
/// With no paths, all of them. Otherwise the given files, and those under the given directories.
/// Untracked files count, unless they are ignored. Deleted files do not.
fn list_files(paths: &[String]) -> Result<Vec<String>, Error> {
    let has_extension = |path: &Path| {
        path.extension()
            .is_some_and(|ext| EXTENSIONS.iter().any(|known| ext == *known))
    };

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    for path in paths {
        let as_path = Path::new(path);
        if as_path.is_dir() {
            dirs.push(path.trim_end_matches('/'));
        } else if as_path.is_file() {
            if has_extension(as_path) {
                files.push(path.strip_prefix("./").unwrap_or(path).to_owned());
            }
        } else {
            return Err(Error::NoSuchPath { path: path.clone() });
        }
    }

    if paths.is_empty() || !dirs.is_empty() {
        let output = std::process::Command::new("git")
            .args([
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
                "--",
            ])
            .args(dirs.iter().map(|dir| format!(":(literal){dir}")))
            .output()
            .map_err(|source| Error::GitSpawn {
                command: "ls-files",
                source,
            })?;
        if !output.status.success() {
            return Err(Error::GitFailed {
                command: "ls-files",
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        files.extend(
            String::from_utf8_lossy(&output.stdout)
                .split('\0')
                .filter(|path| has_extension(Path::new(path)) && Path::new(path).is_file())
                .map(str::to_owned),
        );
    }

    files.sort();
    files.dedup();
    Ok(files)
}

/// The configuration of each file: the `relint.toml` in its nearest ancestor directory, if any.
fn configs_for(files: &[String]) -> Result<Vec<std::sync::Arc<Config>>, Error> {
    let cwd = std::env::current_dir().map_err(Error::CurrentDir)?;
    let mut by_path: HashMap<Option<PathBuf>, std::sync::Arc<Config>> = HashMap::new();
    let mut nearest_by_dir: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();
    files
        .iter()
        .map(|file| {
            let dir = cwd
                .join(file)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default();
            let config_path = nearest_by_dir
                .entry(dir.clone())
                .or_insert_with(|| {
                    dir.ancestors()
                        .map(|ancestor| ancestor.join(config::FILE_NAME))
                        .find(|candidate| candidate.is_file())
                })
                .clone();
            if let Some(config) = by_path.get(&config_path) {
                return Ok(config.clone());
            }
            let config = match &config_path {
                Some(path) => Config::load(path)?,
                None => Config::default(),
            };
            for rule in &config.disabled_rules {
                if !rules::RULES.iter().any(|known| known.name == rule) {
                    return Err(Error::UnknownRule {
                        rule: rule.clone(),
                        config: config_path
                            .clone()
                            .unwrap_or_else(|| PathBuf::from(config::FILE_NAME)),
                    });
                }
            }
            let config = std::sync::Arc::new(config);
            by_path.insert(config_path, config.clone());
            Ok(config)
        })
        .collect()
}

/// The files that `.gitattributes` marks as `linguist-generated`.
fn generated_files(files: &[String]) -> Result<HashSet<String>, Error> {
    use std::io::Write as _;

    let mut child = std::process::Command::new("git")
        .args(["check-attr", "-z", "--stdin", "linguist-generated"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|source| Error::GitSpawn {
            command: "check-attr",
            source,
        })?;
    let mut stdin = child.stdin.take().ok_or(Error::GitStdin)?;
    let input = files.join("\0") + "\0";
    let writer = std::thread::Builder::new()
        .name("git check-attr stdin".to_owned())
        .spawn(move || stdin.write_all(input.as_bytes()))
        .map_err(Error::SpawnThread)?;
    let output = child.wait_with_output().map_err(|source| Error::GitSpawn {
        command: "check-attr",
        source,
    })?;
    writer
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        .map_err(Error::GitWrite)?;
    if !output.status.success() {
        return Err(Error::GitFailed {
            command: "check-attr",
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    // The output is `path NUL attribute NUL value NUL` for each path.
    let output = String::from_utf8_lossy(&output.stdout);
    let fields: Vec<&str> = output.split('\0').collect();
    Ok(fields
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|[_, _, value]| *value == "true" || *value == "set")
        .map(|[path, _, _]| (*path).to_owned())
        .collect())
}

/// The formatted diagnostics of each file, in the order of `files`.
fn lint_files_in_parallel(
    files: &[String],
    configs: &[std::sync::Arc<Config>],
    fix: bool,
) -> Vec<Result<Vec<String>, Error>> {
    let num_threads = std::thread::available_parallelism().map_or(1, usize::from);
    let next_file = AtomicUsize::new(0);
    let mut outputs: Vec<(usize, Result<Vec<String>, Error>)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..num_threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut outputs = Vec::new();
                    loop {
                        let index = next_file.fetch_add(1, Ordering::Relaxed);
                        let Some(path) = files.get(index) else {
                            break;
                        };
                        outputs.push((index, lint_file(path, &configs[index], fix)));
                    }
                    outputs
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| {
                worker
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    outputs.sort_by_key(|(index, _)| *index);
    outputs.into_iter().map(|(_, output)| output).collect()
}

/// Lint one file, and return the formatted diagnostics.
///
/// With `fix`, apply the automatic fixes to the file, and only return the other diagnostics.
fn lint_file(path: &str, config: &Config, fix: bool) -> Result<Vec<String>, Error> {
    let text = std::fs::read_to_string(path).map_err(|source| Error::ReadFile {
        path: path.to_owned(),
        source,
    })?;
    let is_enabled = |rule: &rules::Rule| {
        !config
            .disabled_rules
            .iter()
            .any(|disabled| disabled == rule.name)
    };
    let known_rules: Vec<&str> = rules::RULES.iter().map(|rule| rule.name).collect();
    let code_block_rules: Vec<&rules::Rule> = rules::RULES
        .iter()
        .filter(|rule| rule.check_markdown && is_enabled(rule))
        .collect();
    let code_block_unchecked: Vec<String> = rules::RULES
        .iter()
        .filter(|rule| !rule.check_markdown || !is_enabled(rule))
        .map(|rule| rule.name.to_owned())
        .collect();
    let code_blocks = code_blocks::Context {
        path,
        rules: &code_block_rules,
        known_rules: &known_rules,
        unchecked_rules: &code_block_unchecked,
    };

    let file = SourceFile::parse(path.to_owned(), text);
    let mut diagnostics = if Path::new(path).extension().is_some_and(|ext| ext == "md") {
        code_blocks::lint_markdown(&code_blocks, &file.text)
    } else {
        let enabled_rules: Vec<&rules::Rule> = rules::RULES
            .iter()
            .filter(|rule| is_enabled(rule))
            .collect();
        let diagnostics = rules::check_file(&file, &enabled_rules);
        let mut diagnostics =
            suppression::apply_markers(&file, diagnostics, &known_rules, &config.disabled_rules);
        diagnostics.extend(code_blocks::lint_doc_comments(&code_blocks, &file.text));
        diagnostics
    };
    diagnostics.sort_by_key(|diagnostic| diagnostic.offset);

    if fix {
        let (fixable, rest): (Vec<_>, Vec<_>) = diagnostics
            .into_iter()
            .partition(|diagnostic| diagnostic.fix.is_some());
        if !fixable.is_empty() {
            apply_fixes(
                path,
                &file.text,
                fixable.iter().filter_map(|d| d.fix.as_ref()),
            )?;
            println!("{path}: fixed {} problem(s)", fixable.len());
        }
        diagnostics = rest;
    }

    Ok(diagnostics
        .into_iter()
        .map(|diagnostic| {
            format!(
                "{}:{}:{}: [{}] {}",
                file.path,
                file.line(diagnostic.offset) + 1,
                file.column(diagnostic.offset),
                diagnostic.rule,
                diagnostic.message,
            )
        })
        .collect())
}

fn apply_fixes<'a>(
    path: &str,
    text: &str,
    fixes: impl Iterator<Item = &'a diagnostic::Fix>,
) -> Result<(), Error> {
    let mut fixes: Vec<&diagnostic::Fix> = fixes.collect();
    fixes.sort_by_key(|fix| std::cmp::Reverse(fix.offset));
    fixes.dedup();
    let mut text = text.to_owned();
    for fix in fixes {
        text.insert_str(usize::from(fix.offset), fix.insert);
    }
    std::fs::write(path, text).map_err(|source| Error::WriteFixes {
        path: path.to_owned(),
        source,
    })
}
