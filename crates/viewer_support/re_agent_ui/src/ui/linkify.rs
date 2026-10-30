//! Agents often write bare URLs. `CommonMark` only links URLs in angle brackets or `[text](url)`,
//! so we wrap bare URLs in angle brackets before rendering.

use std::borrow::Cow;

/// Clicking a `rerun` URL opens it in the viewer.
const SCHEMES: [&str; 5] = [
    "https://",
    "http://",
    "rerun://",
    "rerun+https://",
    "rerun+http://",
];

/// Wraps bare `http(s)://` and `rerun://` URLs in `<…>` so the markdown renderer turns them into links.
///
/// A code span holding nothing but such a URL becomes a link too.
/// Leaves alone any other code spans or fences, and URLs that are already part of a link.
pub fn linkify_bare_urls(markdown: &str) -> Cow<'_, str> {
    if !SCHEMES.iter().any(|scheme| markdown.contains(scheme)) {
        return Cow::Borrowed(markdown);
    }

    let mut out = String::with_capacity(markdown.len() + 16);
    let mut in_code = false;
    let mut rest = markdown;

    while !rest.is_empty() {
        if !in_code && let Some(url) = url_code_span(rest) {
            out.push('<');
            out.push_str(url);
            out.push('>');
            rest = &rest[url.len() + 2..];
            continue;
        }

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

/// The URL in a single-backtick code span at the start of `text`, if the span holds nothing else.
fn url_code_span(text: &str) -> Option<&str> {
    let inner = text.strip_prefix('`')?;
    let end = inner.find('`')?;
    let url = &inner[..end];
    let is_url = SCHEMES.iter().any(|scheme| url.starts_with(scheme))
        && !url.contains(|c: char| c.is_whitespace() || c == '<' || c == '>');
    (is_url && !inner[end + 1..].starts_with('`')).then_some(url)
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
        assert_eq!(
            linkify_bare_urls(
                "Open rerun://host/dataset/abc?segment_id=s1, or rerun+http://localhost:51234/entry/xyz."
            ),
            "Open <rerun://host/dataset/abc?segment_id=s1>, or <rerun+http://localhost:51234/entry/xyz>."
        );
    }

    #[test]
    fn links_code_spans_holding_only_a_url() {
        assert_eq!(
            linkify_bare_urls(
                "Open `rerun://host/dataset/abc?segment_id=s1` or `https://rerun.io`."
            ),
            "Open <rerun://host/dataset/abc?segment_id=s1> or <https://rerun.io>."
        );
    }

    #[test]
    fn leaves_existing_links_and_code_alone() {
        for unchanged in [
            "[docs](https://rerun.io/docs)",
            "<https://rerun.io>",
            "`curl https://rerun.io`",
            "`ftp://rerun.io`",
            "```\ncurl https://rerun.io\n```",
            "```\nhttps://rerun.io\n```",
            "prefix/https://example.com/not-a-link",
        ] {
            assert_eq!(linkify_bare_urls(unchanged), unchanged, "{unchanged}");
        }
    }
}
