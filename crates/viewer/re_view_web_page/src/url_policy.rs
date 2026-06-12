#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UrlPolicyResult {
    Accepted(String),
    Invalid,
    UnsupportedScheme(String),
}

pub(crate) fn validate_url(url: &str) -> UrlPolicyResult {
    match url::Url::parse(url) {
        Ok(parsed_url) => match parsed_url.scheme() {
            "http" | "https" => UrlPolicyResult::Accepted(url.to_owned()),
            scheme => UrlPolicyResult::UnsupportedScheme(scheme.to_owned()),
        },
        Err(_) => UrlPolicyResult::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::{UrlPolicyResult, validate_url};

    #[test]
    fn accepts_http_and_https_urls() {
        assert_eq!(
            validate_url("https://example.com"),
            UrlPolicyResult::Accepted("https://example.com".to_owned())
        );
        assert_eq!(
            validate_url("http://localhost:3000"),
            UrlPolicyResult::Accepted("http://localhost:3000".to_owned())
        );
    }

    #[test]
    fn rejects_unsupported_schemes() {
        assert_eq!(
            validate_url("file:///tmp/report.html"),
            UrlPolicyResult::UnsupportedScheme("file".to_owned())
        );
        assert_eq!(
            validate_url("javascript:alert(1)"),
            UrlPolicyResult::UnsupportedScheme("javascript".to_owned())
        );
        assert_eq!(
            validate_url("data:text/plain,hello"),
            UrlPolicyResult::UnsupportedScheme("data".to_owned())
        );
    }

    #[test]
    fn rejects_invalid_url_text() {
        assert_eq!(validate_url("not a url"), UrlPolicyResult::Invalid);
    }
}
