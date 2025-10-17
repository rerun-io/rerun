use regex_macro::regex;

fn is_valid_todo_part(part: &str) -> bool {
    let part = part.trim();
    regex!(r"^([\w/-]*#\d+|[a-z][a-z0-9_]+|RR-\d+)$").is_match(part)
}

fn check_string(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }

    const BAD_TITLES: &[&str] = &[
        "Blueprint",
        "Class",
        "Container",
        "Entity",
        "EntityPath",
        "Epoch",
        "Instance",
        "Path",
        "Recording",
        "Result",
        "Space",
        "Store",
        "View",
        "Viewport",
    ];

    let pattern = regex!(r"[^.] ([A-Z]\w+)");
    if let Some(cap) = pattern.captures(s) {
        let word = &cap[1];
        if BAD_TITLES.contains(&word) {
            return Some(format!(
                "Do not use title casing ({}). See https://github.com/rerun-io/rerun/blob/main/DESIGN.md",
                word
            ));
        }
    }

    None
}

fn lint_url(url: &str) -> Option<String> {
    const ALLOW_LIST: &[&str] = &[
        "https://github.com/lycheeverse/lychee/blob/master/lychee.example.toml",
        "https://github.com/rerun-io/documentation/blob/main/src/utils/tokens.ts",
        "https://github.com/rerun-io/rerun/blob/main/ARCHITECTURE.md",
        "https://github.com/rerun-io/rerun/blob/main/CODE_OF_CONDUCT.md",
        "https://github.com/rerun-io/rerun/blob/main/CONTRIBUTING.md",
        "https://github.com/rerun-io/rerun/blob/main/LICENSE-APACHE",
        "https://github.com/rerun-io/rerun/blob/main/LICENSE-MIT",
    ];

    if ALLOW_LIST.contains(&url) {
        return None;
    }

    if let Some(cap) = regex!(r"https://github.com/.*/blob/(\w+)/.*").captures(url) {
        let branch = &cap[1];
        if ["main", "master", "trunk", "latest"].contains(&branch) {
            if url.contains("#L") {
                return Some(format!(
                    "Do not link directly to a file:line on '{}' - it may change! Use a perma-link instead (commit hash or tag). Url: {}",
                    branch, url
                ));
            }

            if url.contains("/README.md") {
                // Probably fine
            } else if url.starts_with("https://github.com/rerun-io/rerun/blob/") {
                // TODO(#6077): figure out how we best link to our own code from our docs
            } else {
                return Some(format!(
                    "Do not link directly to a file on '{}' - it may disappear! Use a commit hash or tag instead. Url: {}",
                    branch, url
                ));
            }
        }
    }

    None
}

#[allow(clippy::too_many_lines)]
pub fn lint_line(
    line: &str,
    prev_line: Option<&str>,
    file_extension: &str,
    is_in_docstring: bool,
) -> Option<String> {
    if line.is_empty() {
        return None;
    }

    let prev_line_stripped = prev_line.map(|l| l.trim()).unwrap_or("");

    if line.ends_with(char::is_whitespace) {
        return Some("Trailing whitespace".to_string());
    }

    if line.contains("NOLINT") {
        return None;
    }

    if !matches!(file_extension, "py" | "txt" | "yaml" | "yml") {
        if line.contains("Github") {
            return Some("It's 'GitHub', not 'Github'".to_string());
        }
        if line.contains(" github ") {
            return Some("It's 'GitHub', not 'github'".to_string());
        }
    }

    if regex!(r"[.a-zA-Z]  [a-zA-Z]").is_match(line) && !line.contains(r"\n  ") {
        return Some("Found double space".to_string());
    }

    if regex!(r"\bthe the\b").is_match(&line.to_lowercase()) {
        return Some("Found 'the the'".to_string());
    }

    // Check for double words (cannot use backreferences in Rust regex)
    let words: Vec<&str> = line.split_whitespace().collect();
    for i in 0..words.len().saturating_sub(1) {
        let word1 = words[i].trim_end_matches(&['.', ',', '!', '?', ':', ';'][..]);
        let word2 = words[i + 1];
        if !word1.is_empty() && word1.chars().all(|c| c.is_ascii_lowercase()) {
            if word1 == word2 || (word2.len() > 1 && word1 == word2.trim_end_matches(&['.'][..])) {
                return Some(format!("Found double word: '{}  {}'", word1, word2));
            }
        }
    }

    if let Some(cap) = regex!(r#"https?://[^ )"]+>"#).captures(line) {
        let url = &cap[0];
        if let Some(err) = lint_url(url) {
            return Some(err);
        }
    }

    if !file_extension.is_empty() {
        if regex!(r"[^.]\.\.\.([^\-.0-9a-zA-Z]|$)").is_match(line) {
            let has_quote = line.contains('"') || line.contains('\'');
            if (has_quote && !line.contains("Callable"))
                || (file_extension != "py"
                    && !regex!(r"[\[\]\(\)<>\{\}]?.*\.\.\..*[\[\]\(\)<>\{\}]").is_match(line)
                    && !regex!(r"from \.\.\.").is_match(line)
                    && !regex!(r"^\s*\.\.\.\s*$").is_match(line)
                    && !regex!(r"&\.\.\.").is_match(line))
            {
                return Some("Use … instead of ... (on Mac it's option+;)".to_string());
            }
        }
    }

    if !line.contains("http") {
        if line.contains("2d") && !line.contains("2D") {
            return Some("we prefer '2D' over '2d'".to_string());
        }
        if line.contains("3d") && !line.contains("3D") {
            return Some("we prefer '3D' over '3d'".to_string());
        }
    }

    if line.contains("recording=rec")
        && !line.contains("rr.")
        && !line.contains("recording=rec.to_native()")
        && !line.contains("recording=recording.to_native()")
    {
        return Some(
            "you must cast the RecordingStream first: `recording=recording.to_native()".to_string(),
        );
    }

    if line.contains("FIXME") {
        return Some("we prefer TODO over FIXME".to_string());
    }

    if line.contains("HACK") {
        return Some("we prefer TODO over HACK".to_string());
    }

    if line.contains("todo:") {
        return Some("write 'TODO:' in upper-case".to_string());
    }

    if line.contains("todo!()") {
        return Some(r#"todo!() should be written as todo!("$details")"#.to_string());
    }

    if let Some(cap) = regex!(r"TODO\(([^)]*)\)").captures(line) {
        let parts: Vec<&str> = cap[1].split(',').collect();
        if parts.is_empty() || !parts.iter().all(|p| is_valid_todo_part(p)) {
            return Some(
                "TODOs should be formatted as either TODO(name), TODO(#42) or TODO(org/repo#42)"
                    .to_string(),
            );
        }
    }

    if regex!(r#"TODO([^_"(]|$)"#).is_match(line) {
        return Some("TODO:s should be written as `TODO(yourname): what to do`".to_string());
    }

    if line.contains("{err:?}")
        || line.contains("{err:#?}")
        || regex!(r"\{:#?\?\}.*, err").is_match(line)
    {
        return Some(
            "Format errors with re_error::format or using Display - NOT Debug formatting!"
                .to_string(),
        );
    }

    if line.contains("from attr import dataclass") {
        return Some(
            "Avoid 'from attr import dataclass'; prefer 'from dataclasses import dataclass'"
                .to_string(),
        );
    }

    if regex!(r"Result<.*, anyhow::Error>").is_match(line) {
        return Some("Prefer using anyhow::Result<>".to_string());
    }

    if let Some(cap) = regex!(r"map_err\(\|(\w+)\|")
        .captures(line)
        .or_else(|| regex!(r"Err\((\w+)\)").captures(line))
    {
        let name = &cap[1];
        if matches!(name, "e" | "error") {
            return Some("Errors should be called 'err', '_err' or '_'".to_string());
        }
    }

    if let Some(cap) = regex!(r"else\s*\{\s*return;?\s*\};").captures(line) {
        let matched = &cap[0];
        if matched != "else { return; };" {
            return Some(format!(
                "Use 'else {{ return; }};' instead of '{}'",
                matched
            ));
        }
    }

    if regex!(r"\bWASM\b").is_match(line) {
        return Some("WASM should be written 'Wasm'".to_string());
    }

    if regex!(r"nb_").is_match(line) {
        return Some("Don't use nb_things - use num_things or thing_count instead".to_string());
    }

    if regex!(r#"[^(]\\"\{\w*\}\\""#).is_match(line) {
        return Some("Prefer using {:?} - it will also escape newlines etc".to_string());
    }

    if let Some(cap) = regex!(r#""([^"]*)""#).captures(line) {
        if let Some(err) = check_string(&cap[1]) {
            return Some(err);
        }
    }

    if line.contains("rec_stream") || line.contains("rr_stream") {
        return Some("Instantiated RecordingStreams should be named `rec`".to_string());
    }

    if !is_in_docstring {
        if let Some(cap) =
            regex!(r#"(RecordingStreamBuilder::new|\.init|RecordingStream)\("([^"]*)"#)
                .captures(line)
                .or_else(|| regex!(r#"(rr.script_setup)\(args, "(\w*)"#).captures(line))
        {
            let app_id = &cap[2];
            if !app_id.starts_with("rerun_example_") && app_id != "<your_app_name>" {
                return Some(format!(
                    "All examples should have an app_id starting with 'rerun_example_'. Found '{}'",
                    app_id
                ));
            }
        }
    }

    // Deref impls should be marked #[inline]
    if line.contains("fn deref(&self)") || line.contains("fn deref_mut(&mut self)") {
        if !matches!(prev_line_stripped, "#[inline]" | "#[inline(always)]") {
            return Some("Deref/DerefMut impls should be marked #[inline]".to_string());
        }
    }

    if line.contains("fn as_ref(&self)") || line.contains("fn borrow(&self)") {
        if !matches!(prev_line_stripped, "#[inline]" | "#[inline(always)]") {
            return Some("as_ref/borrow implementations should be marked #[inline]".to_string());
        }
    }

    if line.contains(": &dyn std::any::Any")
        || line.contains(": &mut dyn std::any::Any")
        || line.contains(": &dyn Any")
        || line.contains(": &mut dyn Any")
    {
        return Some(
            "Functions should never take `&dyn std::any::Any` as argument since `&Box<std::any::Any>` itself implements `Any`, making it easy to accidentally pass the wrong object. Expect purpose defined traits instead."
                .to_string(),
        );
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_line() {
        let should_pass = vec![
            "hello world",
            "this is a 2D view",
            "todo lowercase is fine",
            r#"todo!("Macro is ok with text")"#,
            "TODO_TOKEN",
            "TODO(bob):",
            "TODO(bob,alice):",
            "TODO(bob, alice):",
            "TODO(#42):",
            "TODO(#42,#43):",
            "TODO(#42, #43):",
            "TODO(n4m3/w1th-numb3r5#42)",
            "TODO(rust-lang/rust#42):",
            "TODO(rust-lang/rust#42,rust-lang/rust#43):",
            "TODO(rust-lang/rust#42, rust-lang/rust#43):",
            r#"eprintln!("{:?}, {err}", foo)"#,
            r#"eprintln!("{:#?}, {err}", foo)"#,
            r#"eprintln!("{err}")"#,
            r#"eprintln!("{}", err)"#,
            "if let Err(err) = foo",
            "if let Err(_err) = foo",
            "if let Err(_) = foo",
            "map_err(|err| …)",
            "map_err(|_err| …)",
            "map_err(|_| …)",
            "WASM_FOO env var",
            "Wasm",
            "num_instances",
            "instances_count",
            "let Some(foo) = bar else { return; };",
            "{foo:?}",
            r#"ui.label("This is fine. Correct casing.")"#,
            "rec",
            "anyhow::Result<()>",
            "The theme is great",
            "template <typename... Args>",
        ];

        let should_error = vec![
            "this is a 2d view",
            "FIXME",
            "HACK",
            "TODO",
            "TODO:",
            "TODO(42)",
            "TODO(https://github.com/rerun-io/rerun/issues/42)",
            "TODO(bob/alice)",
            "TODO(bob|alice)",
            "todo!()",
            r#"eprintln!("{err:?}")"#,
            r#"eprintln!("{err:#?}")"#,
            r#"eprintln!("{:?}", err)"#,
            r#"eprintln!("{:#?}", err)"#,
            "if let Err(error) = foo",
            "map_err(|e| …)",
            "We use WASM in Rerun",
            "nb_instances",
            "inner_nb_instances",
            r#"ui.label("This uses ugly title casing for View.")"#,
            "trailing whitespace ",
            "rr_stream",
            "rec_stream",
            "Result<(), anyhow::Error>",
            "The the problem with double words",
        ];

        for test in should_pass {
            let err = lint_line(test, None, "rs", false);
            assert!(
                err.is_none(),
                "expected '{}' to pass, but got error: '{:?}'",
                test,
                err
            );
        }

        for test in should_error {
            let err = lint_line(test, None, "rs", false);
            assert!(err.is_some(), "expected '{}' to fail, but it passed", test);
        }
    }
}
