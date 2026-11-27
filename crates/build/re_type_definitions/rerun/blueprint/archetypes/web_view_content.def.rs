// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// What a web view shows.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct WebViewContent {
    /// The URL of the page to load.
    ///
    /// Takes precedence over `html`.
    #[rerun(optional)]
    pub url: Option<rerun::components::Text>,

    /// The page, as raw HTML.
    ///
    /// On the web, the page runs in a sandboxed `<iframe>` with an opaque origin,
    /// and so does every page it navigates to.
    #[rerun(optional)]
    pub html: Option<rerun::components::Text>,
}
