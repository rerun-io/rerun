use std::path::Path;

use super::{Context, DocumentData, DocumentKind};
use crate::build_search_index::util::ProgressBarExt as _;

pub fn ingest(ctx: &Context) -> anyhow::Result<()> {
    let progress = ctx.progress_bar("docs");

    let dir = ctx.workspace_root().join("docs").join("content");
    for entry in glob::glob(&format!("{dir}/**/*.md"))? {
        let entry = entry?;
        let path = entry.strip_prefix(&dir)?;
        // Upcoming changelog entries describe unreleased changes.
        if path.starts_with("changelog/upcoming") {
            continue;
        }

        let path = path.with_extension("").display().to_string();
        progress.set(path.clone(), ctx.is_tty());
        let url = format!("https://rerun.io/docs/{path}");
        let (frontmatter, body) = parse_docs_frontmatter(&entry)?;

        ctx.push(DocumentData {
            kind: DocumentKind::Docs,
            title: frontmatter.title,
            hidden_tags: frontmatter.keywords,
            tags: vec![],
            content: body,
            url,
        });
    }

    ctx.finish_progress_bar(progress);

    Ok(())
}

struct DocsFrontmatter {
    title: String,

    /// Search terms that match the page without being shown on it.
    keywords: Vec<String>,
}

fn find_frontmatter_and_body(path: &Path) -> anyhow::Result<(String, String)> {
    let content = std::fs::read_to_string(path)?;

    const START: &str = "---";
    const END: &str = "---";

    let Some(start) = content.find(START) else {
        anyhow::bail!("{:?} is missing frontmatter", path.display())
    };
    let start = start + START.len();

    let Some(end) = content[start..].find(END) else {
        anyhow::bail!(
            "{:?} has invalid frontmatter: missing {END:?} terminator",
            path.display()
        );
    };
    let end = start + end;

    let frontmatter = content[start..end].trim().to_owned();
    let body = content[end + END.len()..].trim().to_owned();

    Ok((frontmatter, body))
}

fn parse_docs_frontmatter(path: &Path) -> anyhow::Result<(DocsFrontmatter, String)> {
    let (frontmatter, body) = find_frontmatter_and_body(path)?;
    let frontmatter = parse_frontmatter(&frontmatter).map_err(|err| {
        anyhow::anyhow!("Invalid frontmatter: {err}\nFile path: {}", path.display())
    })?;

    Ok((frontmatter, body))
}

fn parse_frontmatter(frontmatter: &str) -> anyhow::Result<DocsFrontmatter> {
    // Fields are parsed by hand to avoid depending on a YAML parser,
    // so only the shapes handled below are supported.

    let Some(title) = frontmatter
        .lines()
        .find_map(|line| field_value(line, "title"))
    else {
        anyhow::bail!("missing `title` field");
    };
    let title = title.trim_matches('"').to_owned();

    let keywords = parse_keywords(frontmatter)?;

    Ok(DocsFrontmatter { title, keywords })
}

/// The value of `line` if it is a top-level `name: value` field.
fn field_value<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    Some(line.strip_prefix(name)?.strip_prefix(':')?.trim())
}

const KEYWORDS_FORMAT: &str = "`keywords` must be a comma-separated string or a list of strings";

/// Reads the `keywords` field, which is one of:
///
/// * a comma-separated string: `keywords: a, b`
/// * an inline list: `keywords: [a, "b, c"]`
/// * a block list: `keywords:` followed by `- a` lines
///
/// Any other YAML shape is an error, so that keywords we can't read are never silently dropped or indexed as garbage.
fn parse_keywords(frontmatter: &str) -> anyhow::Result<Vec<String>> {
    let mut lines = frontmatter.lines();
    let Some(value) = lines
        .by_ref()
        .find_map(|line| field_value(line, "keywords"))
    else {
        return Ok(vec![]);
    };

    let keywords = if value.is_empty() || value.starts_with('#') {
        parse_block_list(lines)?
    } else if let Some(list) = value.strip_prefix('[') {
        parse_inline_list(list)?
    } else if value.starts_with(['{', '|', '>']) {
        anyhow::bail!("{KEYWORDS_FORMAT}");
    } else {
        parse_scalar(value)?
            .split(',')
            .map(str::trim)
            .filter(|keyword| !keyword.is_empty())
            .map(str::to_owned)
            .collect()
    };

    if keywords.is_empty() {
        anyhow::bail!("{KEYWORDS_FORMAT}");
    }

    Ok(keywords)
}

/// Parses the `- item` lines that follow the field name, up to the next top-level field.
fn parse_block_list<'a>(lines: impl Iterator<Item = &'a str>) -> anyhow::Result<Vec<String>> {
    let mut keywords = vec![];

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(item) = trimmed.strip_prefix("- ") {
            let keyword = parse_scalar(item.trim_start())?.trim();
            if keyword.is_empty() {
                anyhow::bail!("{KEYWORDS_FORMAT}");
            }
            keywords.push(keyword.to_owned());
        } else if line.starts_with(char::is_whitespace) || trimmed == "-" {
            anyhow::bail!("{KEYWORDS_FORMAT}");
        } else {
            break;
        }
    }

    Ok(keywords)
}

/// Parses what follows the opening `[` of an inline list.
fn parse_inline_list(list: &str) -> anyhow::Result<Vec<String>> {
    let mut keywords = vec![];
    let mut rest = list.trim_start();

    loop {
        if let Some(after_list) = rest.strip_prefix(']') {
            ensure_only_comment(after_list)?;
            return Ok(keywords);
        }

        let (keyword, after_item) = if let Some(quote) = leading_quote(rest) {
            let Some((keyword, after_item)) = rest[1..].split_once(quote) else {
                anyhow::bail!("{KEYWORDS_FORMAT}");
            };
            (keyword, after_item.trim_start())
        } else {
            let Some(end) = rest.find([',', ']']) else {
                anyhow::bail!("{KEYWORDS_FORMAT}");
            };
            (rest[..end].trim(), &rest[end..])
        };

        if keyword.is_empty() {
            anyhow::bail!("{KEYWORDS_FORMAT}");
        }
        keywords.push(keyword.to_owned());

        rest = match after_item.strip_prefix(',') {
            Some(after_comma) => after_comma.trim_start(),
            None if after_item.starts_with(']') => after_item,
            None => anyhow::bail!("{KEYWORDS_FORMAT}"),
        };
    }
}

/// The content of a plain or quoted scalar, without any trailing `# comment`.
fn parse_scalar(value: &str) -> anyhow::Result<&str> {
    if let Some(quote) = leading_quote(value) {
        let Some((scalar, rest)) = value[1..].split_once(quote) else {
            anyhow::bail!("{KEYWORDS_FORMAT}");
        };
        ensure_only_comment(rest)?;
        Ok(scalar)
    } else {
        let comment_start = if value.starts_with('#') {
            Some(0)
        } else {
            value.find(" #")
        };
        Ok(&value[..comment_start.unwrap_or(value.len())])
    }
}

fn leading_quote(value: &str) -> Option<char> {
    value.chars().next().filter(|c| matches!(c, '"' | '\''))
}

fn ensure_only_comment(rest: &str) -> anyhow::Result<()> {
    let rest = rest.trim_start();
    if rest.is_empty() || rest.starts_with('#') {
        Ok(())
    } else {
        anyhow::bail!("{KEYWORDS_FORMAT}")
    }
}

#[cfg(test)]
mod tests {
    use super::{field_value, parse_frontmatter, parse_keywords};

    fn keywords(value: &str) -> Vec<String> {
        parse_keywords(&format!("keywords: {value}")).unwrap()
    }

    #[test]
    fn title_quotes_are_stripped() {
        let frontmatter = parse_frontmatter("title: \"GraphView\"\norder: 3").unwrap();
        assert_eq!(frontmatter.title, "GraphView");
    }

    #[test]
    fn missing_title_is_an_error() {
        assert!(parse_frontmatter("order: 3").is_err());
    }

    #[test]
    fn field_must_start_its_line() {
        assert_eq!(field_value("title: Real", "title"), Some("Real"));
        assert_eq!(
            field_value("description: The title: of things", "title"),
            None
        );
        assert_eq!(field_value("  title: Nested", "title"), None);
    }

    #[test]
    fn keywords_default_to_empty() {
        let frontmatter = parse_frontmatter("title: GraphView").unwrap();
        assert!(frontmatter.keywords.is_empty());
    }

    #[test]
    fn keywords_are_read_from_frontmatter() {
        let frontmatter = parse_frontmatter("title: GraphView\nkeywords: graph, nodes").unwrap();
        assert_eq!(frontmatter.keywords, ["graph", "nodes"]);
    }

    #[test]
    fn string_splits_on_commas() {
        assert_eq!(keywords("bar chart, histogram"), ["bar chart", "histogram"]);
        assert_eq!(
            keywords("bar chart, , histogram,"),
            ["bar chart", "histogram"]
        );
    }

    #[test]
    fn string_may_be_quoted() {
        assert_eq!(
            keywords("\"bar chart, histogram\""),
            ["bar chart", "histogram"]
        );
        assert_eq!(
            keywords("'bar chart, histogram'"),
            ["bar chart", "histogram"]
        );
    }

    #[test]
    fn string_keeps_apostrophes_and_hashes() {
        assert_eq!(keywords("user's guide, C#"), ["user's guide", "C#"]);
    }

    #[test]
    fn string_ignores_trailing_comment() {
        assert_eq!(keywords("a, b   # Optional: x, y"), ["a", "b"]);
        assert_eq!(keywords("\"a, b\" # Optional: x, y"), ["a", "b"]);
    }

    #[test]
    fn inline_list() {
        assert_eq!(
            keywords("[bar chart, histogram]"),
            ["bar chart", "histogram"]
        );
        assert_eq!(
            keywords("[ bar chart , histogram, ]"),
            ["bar chart", "histogram"]
        );
    }

    #[test]
    fn inline_list_keeps_commas_inside_quotes() {
        assert_eq!(
            keywords("[\"pose, 6-DoF\", 'tf', user's]"),
            ["pose, 6-DoF", "tf", "user's"]
        );
    }

    #[test]
    fn inline_list_ignores_trailing_comment() {
        assert_eq!(keywords("[a, b] # synonyms, more"), ["a", "b"]);
    }

    #[test]
    fn block_list() {
        let frontmatter = "title: T\nkeywords:\n  - bar chart\n  - histogram\norder: 3";
        assert_eq!(
            parse_keywords(frontmatter).unwrap(),
            ["bar chart", "histogram"]
        );
    }

    #[test]
    fn block_list_without_indentation() {
        let frontmatter = "keywords:\n- bar chart\n- histogram\ntitle: T";
        assert_eq!(
            parse_keywords(frontmatter).unwrap(),
            ["bar chart", "histogram"]
        );
    }

    #[test]
    fn block_list_items_are_not_split_on_commas() {
        let frontmatter = "keywords:\n  - \"pose, 6-DoF\"\n  - tf, tf2\n  - 'quoted' # comment";
        assert_eq!(
            parse_keywords(frontmatter).unwrap(),
            ["pose, 6-DoF", "tf, tf2", "quoted"]
        );
    }

    #[test]
    fn block_list_skips_blank_lines_and_comments() {
        let frontmatter = "keywords: # search terms\n  - a\n\n  # more\n  - b # trailing\n";
        assert_eq!(parse_keywords(frontmatter).unwrap(), ["a", "b"]);
    }

    #[test]
    fn block_list_does_not_hide_later_fields() {
        let frontmatter = parse_frontmatter("keywords:\n  - a\ntitle: After").unwrap();
        assert_eq!(frontmatter.title, "After");
        assert_eq!(frontmatter.keywords, ["a"]);
    }

    #[test]
    fn unreadable_keywords_are_an_error() {
        for frontmatter in [
            "keywords:",
            "keywords: # comment",
            "keywords:\ntitle: T",
            "keywords:\n  -\n  - a",
            "keywords:\n  - a\n  nested: b",
            "keywords:\n  - \"a",
            "keywords: {a: b}",
            "keywords: >",
            "keywords: |",
            "keywords: \"a, b",
            "keywords: \"a\" b",
            "keywords: \"\"",
            "keywords: []",
            "keywords: [a, b",
            "keywords: [a, , b]",
            "keywords: [\"a\" b]",
            "keywords: [a] b",
        ] {
            assert!(
                parse_keywords(frontmatter).is_err(),
                "{frontmatter:?} should be rejected"
            );
        }
    }
}
