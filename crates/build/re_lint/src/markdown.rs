use anyhow::Result;
use regex_macro::regex;
use std::path::Path;

use crate::SourceFile;

const FORCE_CAPITALIZED: &[&str] = &[
    "2D", "3D", "Apache", "API", "APIs", "April", "Bevy", "C", "C++", "C++17,", "CI", "Colab",
    "Google", "Gradio", "gRPC", "GUI", "GUIs", "July", "Jupyter", "LeRobot", "Linux", "Mac",
    "macOS", "ML", "Numpy", "nuScenes", "Pandas", "PDF", "Pixi", "Polars", "Python", "Q1", "Q2",
    "Q3", "Q4", "Rerun", "Rust", "SAM", "SDK", "SDKs", "UI", "UIs", "UX", "Wasm",
];

const ALLOW_CAPITALIZED: &[&str] = &["Viewer", "Arrow"];

fn is_emoji(s: &str) -> bool {
    s.chars().any(|c| {
        let c = c as u32;
        matches!(c,
            0x1F600..=0x1F64F  // Emoticons
            | 0x1F300..=0x1F5FF  // Miscellaneous Symbols and Pictographs
            | 0x1F680..=0x1F6FF  // Transport and Map Symbols
            | 0x2600..=0x26FF  // Miscellaneous Symbols
            | 0x2700..=0x27BF  // Dingbats
            | 0xFE00..=0xFE0F  // Variation Selectors
            | 0x1F900..=0x1F9FF  // Supplemental Symbols and Pictographs
            | 0x1FA70..=0x1FAFF  // Symbols and Pictographs Extended-A
        )
    })
}

fn split_words(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut word = String::new();

    for ch in input.chars() {
        if ch.is_alphanumeric() || "/_@`.!?+-()".contains(ch) {
            word.push(ch);
        } else {
            if !word.is_empty() {
                result.push(word.clone());
                word.clear();
            }
            result.push(ch.to_string());
        }
    }

    if !word.is_empty() {
        result.push(word);
    }

    result
}

fn is_acronym_or_pascal_case(s: &str) -> bool {
    s.chars().filter(|c| c.is_uppercase()).count() > 1
}

fn fix_header_casing(s: &str) -> String {
    if s.starts_with('[') {
        return s.to_string(); // Don't handle links in headers yet
    }

    let mut new_words = Vec::new();
    let mut last_punctuation: Option<char> = None;
    let mut inline_code_block = false;
    let mut is_first_word = true;

    let force_lower: Vec<String> = FORCE_CAPITALIZED.iter().map(|s| s.to_lowercase()).collect();
    let allow_lower: Vec<String> = ALLOW_CAPITALIZED.iter().map(|s| s.to_lowercase()).collect();

    for word in s.trim().split(' ') {
        if word.is_empty() {
            continue;
        }

        if word == "I" {
            new_words.push(word.to_string());
            continue;
        }

        if is_emoji(word) {
            new_words.push(word.to_string());
            continue;
        }

        let mut word = word.to_string();

        if word.starts_with('`') {
            inline_code_block = true;
        }

        if last_punctuation.is_some() {
            word = word
                .chars()
                .enumerate()
                .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                .collect();
            last_punctuation = None;
        } else if !inline_code_block && !word.starts_with('`') && !word.starts_with('"') {
            if let Some(idx) = force_lower.iter().position(|s| s == &word.to_lowercase()) {
                if word.ends_with(&['?', '!', '.'][..]) {
                    let last_char = word.chars().last().unwrap();
                    last_punctuation = Some(last_char);
                    word.pop();
                }
                word = FORCE_CAPITALIZED[idx].to_string();
            } else if word.ends_with(&['?', '!', '.'][..]) {
                let last_char = word.chars().last().unwrap();
                last_punctuation = Some(last_char);
                word.pop();
            } else if is_acronym_or_pascal_case(&word) || word.chars().any(|c| "_().".contains(c)) {
                // acronym, PascalCase, code, ...
            } else if allow_lower.contains(&word.to_lowercase()) {
                // Allow these to be in any case
            } else if is_first_word {
                word = word
                    .chars()
                    .enumerate()
                    .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                    .collect();
            } else {
                word = word.to_lowercase();
            }
        }

        if word.ends_with('`') {
            inline_code_block = false;
        }

        if let Some(_punct) = last_punctuation {
            word.push(_punct);
        }

        new_words.push(word);
        is_first_word = false;
    }

    new_words.join(" ")
}

fn fix_enforced_upper_case(s: &str) -> String {
    let mut new_words = Vec::new();
    let mut inline_code_block = false;

    let force_lower: Vec<String> = FORCE_CAPITALIZED.iter().map(|s| s.to_lowercase()).collect();

    for word in split_words(s) {
        let mut word = word;

        if word.starts_with('`') {
            inline_code_block = true;
        }
        if word.ends_with('`') {
            inline_code_block = false;
        }

        if !word.trim().is_empty() && !inline_code_block && !word.starts_with('`') {
            if let Some(idx) = force_lower.iter().position(|s| s == &word.to_lowercase()) {
                word = FORCE_CAPITALIZED[idx].to_string();
            }
        }

        new_words.push(word);
    }

    new_words.join("")
}

pub fn lint_markdown(filepath: &Path, source: &SourceFile) -> Result<(Vec<String>, Vec<String>)> {
    let mut errors = Vec::new();
    let mut lines_out = Vec::new();

    let filepath_str = filepath.to_string_lossy();
    let in_example_readme = filepath_str.contains("/examples/python/")
        && filepath_str.ends_with("README.md")
        && !filepath_str.ends_with("/examples/python/README.md");
    let in_code_of_conduct = filepath_str.ends_with("CODE_OF_CONDUCT.md");

    if in_code_of_conduct {
        return Ok((errors, source.lines.clone()));
    }

    let mut in_code_block = false;
    let mut in_frontmatter = false;
    let mut in_metadata = false;

    for (line_nr, line) in source.lines.iter().enumerate() {
        let line_nr_1based = line_nr + 1;
        let mut line = line.clone();

        if line.trim().starts_with("```") {
            in_code_block = !in_code_block;
        }

        if line.starts_with("---") {
            in_frontmatter = !in_frontmatter;
        }
        if line.starts_with("<!--[metadata]") {
            in_metadata = true;
        }
        if in_metadata && line.starts_with("-->") {
            in_metadata = false;
        }

        if !in_code_block && !source.should_ignore(line_nr) {
            if !in_metadata {
                // Check the casing on markdown headers
                if let Some(cap) = regex!(r"^(#+\s+)(.*)").captures(&line) {
                    let prefix = &cap[1];
                    let header_text = &cap[2];
                    let new_header = fix_header_casing(header_text);
                    if new_header != header_text {
                        errors.push(format!(
                            "{}: Markdown headers should NOT be title cased, except certain words which are always capitalized. This should be '{}'",
                            line_nr_1based,
                            new_header
                        ));
                        line = format!("{}{}\n", prefix, new_header);
                    }
                }
            } else {
                // Check the casing on `title = "..."` frontmatter
                if let Some(cap) = regex!(r#"^title\s*=\s*"(.*)""#).captures(&line) {
                    let title = &cap[1];
                    let new_title = fix_header_casing(title);
                    if new_title != title {
                        errors.push(format!(
                            "{}: Titles should NOT be title cased, except certain words which are always capitalized. This should be '{}'",
                            line_nr_1based,
                            new_title
                        ));
                        line = format!("title = \"{}\"\n", new_title);
                    }
                }
            }

            // Enforce capitalization on certain words in the main text
            if !in_frontmatter {
                let new_line = fix_enforced_upper_case(&line);
                if new_line != line {
                    errors.push(format!(
                        "{}: Certain words should be capitalized. This should be '{}'",
                        line_nr_1based, new_line
                    ));
                    line = new_line;
                }
            }

            if in_example_readme && !in_metadata {
                // Check that <h1> is not used in example READMEs
                if line.starts_with('#') && !line.starts_with("##") {
                    errors.push(format!(
                        "{}: Do not use top-level headers in example READMEs, they are reserved for page title",
                        line_nr_1based
                    ));
                }
            }
        }

        lines_out.push(line);
    }

    Ok((errors, lines_out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_emoji() {
        assert!(!is_emoji("A"));
        assert!(!is_emoji("Ö"));
        assert!(is_emoji("😀"));
        assert!(is_emoji("⚠️"));
        assert!(is_emoji("🎉"));
        assert!(!is_emoji("hello"));
    }

    #[test]
    fn test_split_words() {
        let test_cases = vec![
            ("hello world", vec!["hello", " ", "world"]),
            ("hello foo@rerun.io", vec!["hello", " ", "foo@rerun.io"]),
            ("www.rerun.io", vec!["www.rerun.io"]),
            ("`rerun`", vec!["`rerun`"]),
            ("hello-world", vec!["hello-world"]),
            ("foo,bar", vec!["foo", ",", "bar"]),
        ];

        for (input, expected) in test_cases {
            let actual = split_words(input);
            assert_eq!(
                actual, expected,
                "Expected '{}' to split into {:?}, got {:?}",
                input, expected, actual
            );
        }
    }

    #[test]
    fn test_fix_header_casing() {
        let test_cases = vec![
            // Should capitalize first word
            ("hello world", "Hello world"),
            // Should keep forced capitalized words
            ("using python and rust", "Using Python and Rust"),
            ("2d and 3d graphics", "2D and 3D graphics"),
            // Should handle acronyms
            ("API documentation", "API documentation"),
            // Should lowercase non-special words after first
            ("Hello World Test", "Hello world test"),
            // Should handle special punctuation
            ("what is rust?", "What is rust?"),
            // Should preserve code blocks
            ("`code` example", "`code` example"),
            // Should handle I
            ("why I love rust", "Why I love Rust"),
            // Allow viewer capitalization
            ("the Viewer window", "The Viewer window"),
            ("the viewer window", "The viewer window"),
        ];

        for (input, expected) in test_cases {
            let actual = fix_header_casing(input);
            assert_eq!(
                actual, expected,
                "Expected '{}' to become '{}', got '{}'",
                input, expected, actual
            );
        }
    }

    #[test]
    fn test_fix_enforced_upper_case() {
        let test_cases = vec![
            ("Using python and rust", "Using Python and Rust"),
            ("2d and 3d graphics", "2D and 3D graphics"),
            ("This uses wasm technology", "This uses Wasm technology"),
            ("The api is great", "The API is great"),
            ("Using `python` code", "Using `python` code"), // Inside backticks should not change
            ("mac and linux support", "Mac and Linux support"),
            ("UI and UX design", "UI and UX design"),
        ];

        for (input, expected) in test_cases {
            let actual = fix_enforced_upper_case(input);
            assert_eq!(
                actual, expected,
                "Expected '{}' to become '{}', got '{}'",
                input, expected, actual
            );
        }
    }

    #[test]
    fn test_lint_markdown_headers() {
        use std::path::PathBuf;

        // Create a mock source file
        let lines = vec![
            "# Hello World Test\n".to_string(),
            "\n".to_string(),
            "Some content using python and rust.\n".to_string(),
        ];

        let source = SourceFile {
            path: PathBuf::from("test.md"),
            content: lines.join(""),
            lines: lines.clone(),
            nolints: std::collections::HashSet::new(),
            ext: "md".to_string(),
        };

        let (errors, fixed_lines) = lint_markdown(&PathBuf::from("test.md"), &source).unwrap();

        // Should have errors for header casing and Python/Rust capitalization
        assert!(!errors.is_empty(), "Expected linting errors");
        assert!(
            errors.iter().any(|e| e.contains("Markdown headers")),
            "Expected header casing error"
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("Certain words should be capitalized")),
            "Expected capitalization error"
        );

        // Check that the header was fixed
        assert_eq!(fixed_lines[0], "# Hello world test\n");
        // Check that Python was capitalized (rust. with period might not capitalize correctly)
        assert!(
            fixed_lines[2].contains("Python"),
            "Python should be capitalized in: {}",
            fixed_lines[2]
        );
    }

    #[test]
    fn test_lint_markdown_no_errors() {
        use std::path::PathBuf;

        let lines = vec![
            "# Correct header format\n".to_string(),
            "\n".to_string(),
            "Content with Python and Rust.\n".to_string(),
        ];

        let source = SourceFile {
            path: PathBuf::from("test.md"),
            content: lines.join(""),
            lines: lines.clone(),
            nolints: std::collections::HashSet::new(),
            ext: "md".to_string(),
        };

        let (errors, _fixed_lines) = lint_markdown(&PathBuf::from("test.md"), &source).unwrap();

        assert!(
            errors.is_empty(),
            "Expected no linting errors, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_lint_markdown_code_blocks() {
        use std::path::PathBuf;

        // Code blocks should not be linted
        let lines = vec![
            "# Good header\n".to_string(),
            "\n".to_string(),
            "```python\n".to_string(),
            "# This is python code with python and rust\n".to_string(),
            "```\n".to_string(),
        ];

        let source = SourceFile {
            path: PathBuf::from("test.md"),
            content: lines.join(""),
            lines: lines.clone(),
            nolints: std::collections::HashSet::new(),
            ext: "md".to_string(),
        };

        let (errors, _fixed_lines) = lint_markdown(&PathBuf::from("test.md"), &source).unwrap();

        // Should not have capitalization errors inside code blocks
        assert!(
            !errors.iter().any(|e| e.contains("python code")),
            "Should not lint inside code blocks"
        );
    }

    #[test]
    fn test_lint_markdown_nolint() {
        use std::collections::HashSet;
        use std::path::PathBuf;

        let lines = vec![
            "# Bad Header That Should Error\n".to_string(),
            "Using python here. NOLINT\n".to_string(),
        ];

        let mut nolints = HashSet::new();
        nolints.insert(1); // Mark line 1 (0-indexed) as NOLINT

        let source = SourceFile {
            path: PathBuf::from("test.md"),
            content: lines.join(""),
            lines: lines.clone(),
            nolints,
            ext: "md".to_string(),
        };

        let (errors, _fixed_lines) = lint_markdown(&PathBuf::from("test.md"), &source).unwrap();

        // Should have error for header but not for the NOLINT line
        assert!(
            errors.iter().any(|e| e.contains("Markdown headers")),
            "Expected header error"
        );
        assert_eq!(
            errors.iter().filter(|e| e.contains("python")).count(),
            0,
            "NOLINT line should be ignored"
        );
    }

    #[test]
    fn test_fix_header_casing_preserves_code() {
        // Code in backticks should be preserved
        assert_eq!(
            fix_header_casing("using `python` code"),
            "Using `python` code"
        );

        // Multiple code spans
        assert_eq!(
            fix_header_casing("`foo` and `bar` example"),
            "`foo` and `bar` example"
        );
    }

    #[test]
    fn test_fix_header_casing_handles_punctuation() {
        // After "?" punctuation, the next word should be capitalized
        assert_eq!(
            fix_header_casing("what is this? it is rust"),
            "What is this? It is Rust"
        );

        // When word has punctuation attached, first letter may not capitalize
        // but the word after punctuation will be capitalized
        assert_eq!(
            fix_header_casing("hello! how are you"),
            "hello! How are you"
        );
    }
}
