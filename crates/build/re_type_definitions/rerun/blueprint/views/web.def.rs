// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A view that shows a web page.
///
/// The page is either loaded from a URL, or given as raw HTML in the blueprint.
/// On the web, the page is shown in an `<iframe>`, so sites that forbid being framed don't load.
///
/// \example views/web title="Use a blueprint to show a web page."
#[rerun::rerun_type]
#[docs(no_data)]
#[docs(keywords = "web, browser, page, url, html, iframe")]
#[rerun(view_identifier = "Web")]
#[rerun(experimental)]
#[rerun(state = "unstable")]
pub struct WebView {
    /// What the page shows.
    pub content: rerun::blueprint::archetypes::WebViewContent,
}
