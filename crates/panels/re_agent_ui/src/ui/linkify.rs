//! Agents often write bare URLs. `CommonMark` only links URLs in angle brackets or `[text](url)`,
//! so we wrap bare URLs in angle brackets before rendering.

use std::borrow::Cow;

const SCHEMES: [&str; 2] = ["https://", "http://"];

/// Wraps bare `http(s)://` URLs in `<…>` so the markdown renderer turns them into links.
///
/// Leaves alone anything inside code spans or fences, and URLs that are already part of a link.
pub fn linkify_bare_urls(markdown: &str) -> Cow<'_, str> {
    if !SCHEMES.iter().any(|scheme| markdown.contains(scheme)) {
        return Cow::Borrowed(markdown);
    }

    let mut out = String::with_capacity(markdown.len() + 16);
    let mut in_code = false;
    let mut rest = markdown;

    while !rest.is_empty() {
        if let Some(stripped) = rest.strip_prefix('`') {
            in_code = !in_code;
            out.push('`');
            rest = stripped;
            continue;
        }

        let starts_url = !in_code
            && SCHEMES.iter().any(|scheme| rest.starts_with(scheme))
            && out.chars().last().is_none_or(is_url_boundary);
        if !starts_url {
            let mut chars = rest.chars();
            if let Some(c) = chars.next() {
                out.push(c);
            }
            rest = chars.as_str();
            continue;
        }

        let end = rest
            .find(|c: char| c.is_whitespace() || c == '<' || c == '>')
            .unwrap_or(rest.len());
        let url = trim_trailing_punctuation(&rest[..end]);
        out.push('<');
        out.push_str(url);
        out.push('>');
        rest = &rest[url.len()..];
    }

    Cow::Owned(out)
}

/// A URL may start here if what came before is not part of a word or of markdown link syntax.
fn is_url_boundary(prev: char) -> bool {
    !(prev.is_alphanumeric() || matches!(prev, '<' | '(' | '"' | '\'' | '/' | ':' | '@'))
}

/// Sentence punctuation right after a URL is not part of it, and neither is an unbalanced `)`.
fn trim_trailing_punctuation(url: &str) -> &str {
    let mut url = url.trim_end_matches(['.', ',', ';', ':', '!', '?', '"', '\'']);
    while url.ends_with(')') && url.matches(')').count() > url.matches('(').count() {
        url = &url[..url.len() - 1];
        url = url.trim_end_matches(['.', ',', ';', ':', '!', '?']);
    }
    url
}

#[cfg(test)]
mod tests {
    use super::linkify_bare_urls;

    #[test]
    fn wraps_bare_urls() {
        assert_eq!(linkify_bare_urls("no links here"), "no links here");
        assert_eq!(
            linkify_bare_urls("see https://claude.ai/settings/connectors"),
            "see <https://claude.ai/settings/connectors>"
        );
        assert_eq!(
            linkify_bare_urls("Go to https://rerun.io. Then http://a.b/c?d=1&e=2, ok?"),
            "Go to <https://rerun.io>. Then <http://a.b/c?d=1&e=2>, ok?"
        );
        assert_eq!(
            linkify_bare_urls("(see https://example.com/wiki/Foo_(bar))"),
            "(see <https://example.com/wiki/Foo_(bar)>)"
        );
    }

    #[test]
    fn leaves_existing_links_and_code_alone() {
        for unchanged in [
            "[docs](https://rerun.io/docs)",
            "<https://rerun.io>",
            "`https://rerun.io`",
            "```\ncurl https://rerun.io\n```",
            "prefix/https://example.com/not-a-link",
        ] {
            assert_eq!(linkify_bare_urls(unchanged), unchanged, "{unchanged}");
        }
    }
}
