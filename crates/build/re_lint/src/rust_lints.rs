use regex_macro::regex;

fn is_empty_or_special(line: &str) -> bool {
    line.is_empty()
        || line.starts_with('#')
        || line.starts_with("//")
        || line.ends_with('{')
        || line.ends_with('(')
        || line.ends_with('\\')
        || line.ends_with("r\"")
        || line.ends_with("r#\"")
        || line.ends_with(']')
}

fn is_missing_blank_line_between(prev_line: &str, line: &str) -> bool {
    if regex!(r"^\s*((pub(\(\w*\))? )?(async )?((impl|fn|struct|enum|union|trait|type)\b))")
        .is_match(line)
        || regex!(r"^\s*#\[(error|derive|inline)").is_match(line)
        || regex!(r"^\s*///").is_match(line)
    {
        let line_trimmed = line.trim();
        let prev_trimmed = prev_line.trim();

        if prev_trimmed.contains("template<") {
            return false; // C++ template inside Rust code
        }

        if is_empty_or_special(prev_trimmed) || prev_trimmed.starts_with("```") {
            return false;
        }

        if line_trimmed.starts_with("fn ") && line_trimmed.ends_with(';') {
            return false; // maybe a trait function
        }

        if line_trimmed.starts_with("type ") && prev_trimmed.ends_with(';') {
            return false; // many type declarations in a row is fine
        }

        if prev_trimmed.ends_with(',') && line_trimmed.starts_with("impl") {
            return false;
        }

        if prev_trimmed.ends_with('*') {
            return false; // maybe in a macro
        }

        if prev_trimmed.ends_with("r##\"") {
            return false; // part of a multi-line string
        }

        return true;
    }

    false
}

pub fn lint_vertical_spacing(lines_in: &[String]) -> (Vec<String>, Vec<String>) {
    let mut errors = Vec::new();
    let mut lines_out = Vec::new();
    let mut prev_line: Option<&String> = None;

    for (line_nr, line) in lines_in.iter().enumerate() {
        let line_nr = line_nr + 1;

        if let Some(prev) = prev_line {
            if is_missing_blank_line_between(prev, line) {
                errors.push(format!(
                    "{}: for readability, add newline before `{}`",
                    line_nr,
                    line.trim()
                ));
                lines_out.push(String::new());
            }
        }

        lines_out.push(line.clone());
        prev_line = Some(line);
    }

    (errors, lines_out)
}

pub fn lint_pyclass_eq(lines_in: &[String]) -> (Vec<String>, Vec<usize>) {
    let mut errors = Vec::new();
    let mut error_linenumbers = Vec::new();
    let mut i = 0;

    while i < lines_in.len() {
        let line = &lines_in[i];
        let line_nr = i + 1;

        // Check if this line starts a pyclass declaration
        if regex!(r"#\[pyclass\(").is_match(line.trim()) {
            // Collect the entire pyclass declaration (might span multiple lines)
            let mut pyclass_content = line.clone();
            let original_line_nr = line_nr;

            // Keep reading lines until we find the closing parenthesis
            let mut paren_count =
                line.matches('(').count() as i32 - line.matches(')').count() as i32;
            let mut j = i + 1;

            while paren_count > 0 && j < lines_in.len() {
                let next_line = &lines_in[j];
                pyclass_content.push_str(next_line);
                paren_count +=
                    next_line.matches('(').count() as i32 - next_line.matches(')').count() as i32;
                j += 1;
            }

            // Check if 'eq' is present in the pyclass declaration
            if !regex!(r"\beq\b").is_match(&pyclass_content) {
                errors.push(format!(
                    "{}: #[pyclass(...)] should include 'eq' parameter for Python equality support",
                    original_line_nr
                ));
                error_linenumbers.push(original_line_nr);
            }

            i = j;
        } else {
            i += 1;
        }
    }

    (errors, error_linenumbers)
}

pub fn lint_workspace_lints(cargo_content: &str) -> Option<&'static str> {
    if regex!(r"\[lints\]\nworkspace\s*=\s*true").is_match(cargo_content) {
        None
    } else {
        Some("Non-example cargo files should have a [lints] section with workspace = true")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_vertical_spacing() {
        let should_pass = vec![
            "hello world",
            "/// docstring\nfoo\n\n/// docstring\nbar",
            "trait Foo {\n    fn bar();\n    fn baz();\n}",
            "$(#[$meta])*\n#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]",
            "Item = (\n    &PointCloudBatchInfo,\n    impl Iterator<Item = &PointCloudVertex>,\n),",
            "type Response = Response<Body>;\ntype Error = hyper::Error;",
            "template<typename T>\nstruct AsComponents;",
        ];

        let should_fail = vec![
            "/// docstring\nfoo\n/// docstring\nbar",
            "Foo,\n#[error]\nBar,",
            "slotmap::new_key_type! { pub struct ViewBuilderHandle; }\ntype ViewBuilderMap = slotmap::SlotMap<ViewBuilderHandle, ViewBuilder>;",
            "fn foo() {}\nfn bar() {}",
            "async fn foo() {}\nasync fn bar() {}",
        ];

        for test in should_pass {
            let lines: Vec<String> = test.split('\n').map(|s| s.to_string()).collect();
            let (errors, _) = lint_vertical_spacing(&lines);
            assert!(
                errors.is_empty(),
                "expected this to pass:\n{}\ngot: {:?}",
                test,
                errors
            );
        }

        for test in should_fail {
            let lines: Vec<String> = test.split('\n').map(|s| s.to_string()).collect();
            let (errors, _) = lint_vertical_spacing(&lines);
            assert!(!errors.is_empty(), "expected this to fail:\n{}", test);
        }
    }

    #[test]
    fn test_lint_pyclass_eq() {
        let should_pass = vec![
            "#[pyclass(eq)]",
            "#[pyclass(frozen, eq, hash)]",
            "#[pyclass(eq, frozen)]",
            "#[pyclass(\n    frozen,\n    eq,\n    hash\n)]",
            "#[pyclass(frozen, hash, eq)]",
            r#"#[pyclass(eq, module = "rerun_bindings.rerun_bindings")]"#,
        ];

        let should_error = vec![
            "#[pyclass(frozen)]",
            "#[pyclass(frozen, hash)]",
            r#"#[pyclass(module = "rerun_bindings.rerun_bindings")]"#,
            "#[pyclass(\n    frozen,\n    hash\n)]",
        ];

        for test_case in should_pass {
            let lines: Vec<String> = test_case.split('\n').map(|s| s.to_string()).collect();
            let (errors, _) = lint_pyclass_eq(&lines);
            assert!(
                errors.is_empty(),
                "expected '{}' to pass, but got errors: {:?}",
                test_case,
                errors
            );
        }

        for test_case in should_error {
            let lines: Vec<String> = test_case.split('\n').map(|s| s.to_string()).collect();
            let (errors, _) = lint_pyclass_eq(&lines);
            assert!(
                !errors.is_empty(),
                "expected '{}' to fail, but got no errors",
                test_case
            );
        }
    }
}
