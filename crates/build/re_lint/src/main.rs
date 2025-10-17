#!/usr/bin/env rust-script

//! Custom linter for the Rerun codebase.
//!
//! Adding "NOLINT" to any line makes the linter ignore that line.
//! Adding a pair of "NOLINT_START" and "NOLINT_END" makes the linter ignore those lines
//! and all lines in between.

mod lint_rules;
mod markdown;
mod rust_lints;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(about = "Lint code with custom linter", long_about = None)]
struct Args {
    /// File paths. Empty = all files, recursively.
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,

    /// Automatically fix some problems
    #[arg(long)]
    fix: bool,

    /// Run some extra checks
    #[arg(long)]
    extra: bool,
}

/// Represents a source file with utilities for linting.
struct SourceFile {
    path: PathBuf,
    content: String,
    lines: Vec<String>,
    nolints: HashSet<usize>,
    ext: String,
}

impl SourceFile {
    fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;

        // Split into lines while preserving line endings
        let mut lines = Vec::new();
        let mut start = 0;
        for (idx, _) in content.match_indices('\n') {
            lines.push(content[start..=idx].to_string());
            start = idx + 1;
        }
        // Handle last line if it doesn't end with newline
        if start < content.len() {
            lines.push(content[start..].to_string());
        }

        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let mut file = Self {
            path: path.to_path_buf(),
            content,
            lines: lines.clone(),
            nolints: HashSet::new(),
            ext,
        };

        file.update_nolints();
        Ok(file)
    }

    fn update_nolints(&mut self) {
        self.nolints.clear();
        let mut in_nolint_block = false;

        for (i, line) in self.lines.iter().enumerate() {
            if line.contains("NOLINT") {
                self.nolints.insert(i);
            }

            if line.contains("NOLINT_START") {
                in_nolint_block = true;
            }

            if in_nolint_block {
                self.nolints.insert(i);
                if line.contains("NOLINT_END") {
                    in_nolint_block = false;
                }
            }
        }
    }

    fn should_ignore(&self, line_nr: usize) -> bool {
        // Check current line and previous line
        self.nolints.contains(&line_nr) || (line_nr > 0 && self.nolints.contains(&(line_nr - 1)))
    }

    fn rewrite(&mut self, new_lines: Vec<String>) -> Result<()> {
        if new_lines != self.lines {
            let new_content = new_lines.join("\n") + "\n";
            std::fs::write(&self.path, &new_content)?;
            self.content = new_content;
            self.lines = new_lines;
            self.update_nolints();
            println!("{} fixed.", self.path.display());
        }
        Ok(())
    }

    fn error(&self, message: &str, line_nr: Option<usize>) -> String {
        if let Some(line) = line_nr {
            format!("{}:{}: {}", self.path.display(), line + 1, message)
        } else {
            format!("{}: {}", self.path.display(), message)
        }
    }
}

fn lint_file(path: &Path, args: &Args) -> Result<usize> {
    let mut source = SourceFile::load(path)?;
    let mut num_errors = 0;
    let mut is_in_docstring = false;
    let mut prev_line: Option<String> = None;

    // Line-by-line linting
    for (line_nr, line) in source.lines.iter().enumerate() {
        if source.should_ignore(line_nr) {
            prev_line = Some(line.clone());
            continue;
        }

        let error = if line.is_empty() || !line.ends_with('\n') {
            // Only report missing newline for the very last line
            if line_nr == source.lines.len() - 1 && !line.is_empty() {
                Some("Missing newline at end of file".to_string())
            } else {
                None
            }
        } else {
            // Remove newline for linting
            let line_content = line.strip_suffix('\n').unwrap_or(line);
            let trimmed = line_content.trim();
            if trimmed == "\"\"\"" {
                is_in_docstring = !is_in_docstring;
            }
            lint_rules::lint_line(
                line_content,
                prev_line.as_deref().and_then(|s| s.strip_suffix('\n')),
                &source.ext,
                is_in_docstring,
            )
        };

        if let Some(err) = error {
            num_errors += 1;
            println!("{}", source.error(&err, Some(line_nr)));
        }

        prev_line = Some(line.clone());
    }

    // File-type specific linting
    if path.extension().map_or(false, |e| e == "hpp") {
        if !source.lines.iter().any(|l| l.starts_with("#pragma once")) {
            println!(
                "{}",
                source.error("Missing `#pragma once` in C++ header file", None)
            );
            num_errors += 1;
        }
    }

    if matches!(source.ext.as_str(), "rs" | "fbs") {
        let (errors, lines_out) = rust_lints::lint_vertical_spacing(&source.lines);
        for error in &errors {
            println!("{}", source.error(error, None));
        }
        num_errors += errors.len();

        // Check for pyclass eq parameter in rerun_py Rust files
        if path.starts_with("./rerun_py/") && source.ext == "rs" {
            let (pyclass_errors, error_lines) = rust_lints::lint_pyclass_eq(&source.lines);
            for (error, line_number) in pyclass_errors.iter().zip(error_lines.iter()) {
                if !source.should_ignore(*line_number) {
                    println!("{}", source.error(error, None));
                    num_errors += 1;
                }
            }
        }

        if args.fix {
            source.rewrite(lines_out)?;
        }
    }

    if source.ext == "md" {
        let (errors, lines_out) = markdown::lint_markdown(path, &source)?;

        for error in &errors {
            println!("{}", source.error(error, None));
        }
        num_errors += errors.len();

        if args.fix {
            source.rewrite(lines_out)?;
        } else if !errors.is_empty() {
            println!(
                "Run with --fix to automatically fix {} errors.",
                errors.len()
            );
        }
    }

    // Cargo.toml workspace lints
    if !path.starts_with("./examples/rust")
        && path.file_name() != Some(std::ffi::OsStr::new("Cargo.toml"))
        && path.extension().map_or(false, |e| e == "toml")
        && path.ends_with("Cargo.toml")
    {
        if let Some(error) = rust_lints::lint_workspace_lints(&source.content) {
            println!("{}", source.error(error, None));
            num_errors += 1;
        }
    }

    Ok(num_errors)
}

fn should_lint_file(path: &Path) -> bool {
    const EXTENSIONS: &[&str] = &[
        "c", "cpp", "fbs", "h", "hpp", "html", "js", "md", "py", "rs", "sh", "toml", "txt", "wgsl",
        "yaml", "yml",
    ];

    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        EXTENSIONS.contains(&ext)
    } else {
        false
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let error_count = AtomicUsize::new(0);

    // Determine the root directory
    let root = std::env::current_dir()?;

    if !args.files.is_empty() {
        // Lint specified files
        for file in &args.files {
            let file_path = if file.is_absolute() {
                file.clone()
            } else {
                root.join(file)
            };

            if should_lint_file(&file_path) {
                match lint_file(&file_path, &args) {
                    Ok(errors) => {
                        error_count.fetch_add(errors, Ordering::SeqCst);
                    }
                    Err(err) => {
                        eprintln!("Error linting {}: {}", file_path.display(), err);
                    }
                }
            }
        }
    } else {
        // Walk all files using the ignore crate
        let walker = ignore::WalkBuilder::new(&root)
            .hidden(false)
            .git_ignore(true)
            .build();

        for entry in walker {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.is_file() && should_lint_file(path) {
                        match lint_file(path, &args) {
                            Ok(errors) => {
                                error_count.fetch_add(errors, Ordering::SeqCst);
                            }
                            Err(err) => {
                                eprintln!("Error linting {}: {}", path.display(), err);
                            }
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Error walking directory: {}", err);
                }
            }
        }

        // Run global lints
        if let Err(err) = lint_crate_docs(&root) {
            eprintln!("Error checking crate docs: {}", err);
        }
    }

    let total_errors = error_count.load(Ordering::SeqCst);
    if total_errors == 0 {
        println!("Linting finished without errors");
        Ok(())
    } else {
        println!("Linting found {} errors.", total_errors);
        std::process::exit(1);
    }
}

fn lint_crate_docs(root: &Path) -> Result<()> {
    use regex_macro::regex;
    use std::collections::HashMap;

    let crates_dir = root.join("crates");
    let architecture_md = root.join("ARCHITECTURE.md");

    if !architecture_md.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&architecture_md)?;

    // Extract all crate names from ARCHITECTURE.md
    let mut listed_crates: HashMap<String, usize> = HashMap::new();
    for (i, line) in content.lines().enumerate() {
        for cap in regex!(r"\bre_\w+").captures_iter(line) {
            let crate_name = cap[0].to_string();
            listed_crates.entry(crate_name).or_insert(i + 1);
        }
    }

    let mut error_count = 0;

    // Find all Cargo.toml files in crates directory
    if crates_dir.exists() {
        for entry in walkdir::WalkDir::new(&crates_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "Cargo.toml" {
                let crate_name = entry
                    .path()
                    .parent()
                    .and_then(|p| p.file_name().and_then(|n| n.to_str()));

                if let Some(name) = crate_name {
                    listed_crates.remove(name);

                    if !content.contains(name) {
                        println!(
                            "{}: missing documentation for crate {}",
                            architecture_md.display(),
                            name
                        );
                        error_count += 1;
                    }
                }
            }
        }
    }

    // Report crates mentioned but not found
    for (crate_name, line_nr) in listed_crates {
        println!(
            "{}:{}: crate name {} does not exist",
            architecture_md.display(),
            line_nr,
            crate_name
        );
        error_count += 1;
    }

    if error_count > 0 {
        std::process::exit(1);
    }

    Ok(())
}
