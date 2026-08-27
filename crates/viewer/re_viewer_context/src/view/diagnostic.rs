/// Severity of a viewer diagnostic.
///
/// Sorts from least concern to highest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, re_byte_size::SizeBytes)]
pub enum ViewerReportSeverity {
    /// Supplemental information that does not require action.
    Info,

    /// A problem that allows the viewer to continue, possibly with degraded output or a fallback.
    Warning,

    /// A problem that prevents the requested output from being shown.
    Error,
}

/// A user-facing diagnostic emitted by the viewer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, re_byte_size::SizeBytes)]
pub struct ViewerDiagnostic {
    pub severity: ViewerReportSeverity,

    /// Short message suitable for inline display.
    pub summary: String,

    /// Optional detailed explanation.
    pub details: Option<String>,
}
