use std::sync::Arc;

use egui_webview::url::{Host, ParseError, Url};
use re_sdk_types::blueprint::archetypes::WebViewContent;
use re_sdk_types::components::Text;
use re_sdk_types::{View as _, ViewClassIdentifier};
use re_ui::Help;
use re_viewer_context::{
    ViewClass, ViewClassRegistryError, ViewQuery, ViewSpawnHeuristics, ViewState,
    ViewSystemExecutionError, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

/// A view that shows a web page, loaded from a URL or given as raw HTML in the blueprint.
#[derive(Default)]
pub struct WebView;

type ViewType = re_sdk_types::blueprint::views::WebView;

/// Why [`parse_url`] rejected a URL.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
enum UrlError {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error("only http and https URLs can be shown, not {scheme:?}")]
    UnsupportedScheme { scheme: String },
}

impl ViewClass for WebView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Web"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_WEB
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("Web view")
            .docs_link("https://rerun.io/docs/reference/types/views/web_view")
            .markdown("Shows a web page, loaded from a URL or given as raw HTML in the blueprint.")
    }

    fn on_register(
        &self,
        _system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        Ok(())
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        _ctx: &ViewerContext<'_>,
        _spawn_context: re_viewer_context::ViewSpawnContext,
    ) -> ViewSpawnHeuristics {
        ViewSpawnHeuristics::empty()
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        _state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        _system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<re_viewer_context::ViewClassUiOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let content = ViewProperty::from_archetype_for_view::<WebViewContent>(ctx, query.view_id);
        let non_empty = |component| -> Result<Option<Text>, ViewSystemExecutionError> {
            Ok(content
                .component_or_empty::<Text>(component)?
                .filter(|text| !text.as_str().is_empty()))
        };

        // A page is closed in the first frame in which its view isn't shown, such as when its
        // tab isn't selected. Showing it again loads it from scratch.
        let webview = egui_webview::WebView::new(egui::Id::unique(("re_view_web", query.view_id)));

        if let Some(url) = non_empty(WebViewContent::descriptor_url().component)? {
            let parsed =
                parse_url(url.as_str()).map_err(|err| ViewSystemExecutionError::InvalidUrl {
                    err: Arc::new(err),
                    url: url.as_str().to_owned(),
                })?;
            webview.url(&parsed).show(ui);
        } else if let Some(html) = non_empty(WebViewContent::descriptor_html().component)? {
            webview.html(html.as_str()).show(ui);
        } else {
            ui.centered_and_justified(|ui| {
                ui.weak("No page. Set the URL or HTML of this view in the selection panel.");
            });
        }

        Ok(Default::default())
    }
}

/// Parses a URL typed by a user.
///
/// Only `http` and `https` URLs are allowed. On the web, a `javascript:` URL would run with the
/// viewer's origin.
///
/// A URL without a scheme gets `https://`, or `http://` if its host is this machine, where
/// servers rarely have a certificate.
fn parse_url(url: &str) -> Result<Url, UrlError> {
    let url = url.trim();
    let parsed = match Url::parse(url) {
        Ok(parsed) if !starts_with_host_and_port(url) => parsed,
        Ok(_) | Err(ParseError::RelativeUrlWithoutBase) => {
            let https = Url::parse(&format!("https://{url}"))?;
            if is_loopback(&https) {
                Url::parse(&format!("http://{url}"))?
            } else {
                https
            }
        }
        Err(err) => return Err(err.into()),
    };
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        scheme => Err(UrlError::UnsupportedScheme {
            scheme: scheme.to_owned(),
        }),
    }
}

/// Whether `url` starts with a host and a port, like `localhost:8080/path`.
///
/// Such a URL parses, but with the host as its scheme.
fn starts_with_host_and_port(url: &str) -> bool {
    url.split_once(':')
        .and_then(|(_host, rest)| rest.split(['/', '?', '#']).next())
        .is_some_and(|port| !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Whether the host of `url` is this machine.
fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(domain)) => domain == "localhost",
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{UrlError, parse_url};

    fn parsed(url: &str) -> Result<String, UrlError> {
        parse_url(url).map(|url| url.to_string())
    }

    #[test]
    fn url_without_scheme_gets_one() {
        assert_eq!(
            parsed(" www.egui.rs "),
            Ok("https://www.egui.rs/".to_owned())
        );
        assert_eq!(
            parsed("egui.rs/path?q=1"),
            Ok("https://egui.rs/path?q=1".to_owned())
        );
        assert_eq!(
            parsed("egui.rs:8080/path"),
            Ok("https://egui.rs:8080/path".to_owned())
        );
        assert_eq!(parsed("http://egui.rs"), Ok("http://egui.rs/".to_owned()));
    }

    #[test]
    fn local_host_without_scheme_gets_http() {
        assert_eq!(
            parsed("localhost:8080"),
            Ok("http://localhost:8080/".to_owned())
        );
        assert_eq!(
            parsed("localhost:443/path"),
            Ok("http://localhost:443/path".to_owned())
        );
        assert_eq!(
            parsed("127.0.0.1:8000"),
            Ok("http://127.0.0.1:8000/".to_owned())
        );
        assert_eq!(parsed("[::1]:8000"), Ok("http://[::1]:8000/".to_owned()));
        assert_eq!(
            parsed("https://localhost:8080"),
            Ok("https://localhost:8080/".to_owned())
        );
    }

    #[test]
    fn only_http_and_https_are_allowed() {
        for (url, scheme) in [
            ("javascript:alert(1)", "javascript"),
            ("data:text/html,<script>alert(1)</script>", "data"),
            ("file:///etc/passwd", "file"),
            ("about:blank", "about"),
            ("mailto:someone@example.com", "mailto"),
        ] {
            assert_eq!(
                parsed(url),
                Err(UrlError::UnsupportedScheme {
                    scheme: scheme.to_owned()
                }),
                "{url:?} must be rejected"
            );
        }
        assert!(parsed("").is_err(), "an empty URL must be rejected");
    }
}
