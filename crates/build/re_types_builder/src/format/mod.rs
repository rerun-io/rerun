/// Implements the formatting pass.
pub trait CodeFormatter {
    /// Formats generated files in-place.
    fn format(&mut self, reporter: &crate::Reporter, files: &mut crate::GeneratedFiles);
}

pub struct NoopCodeFormatter;

impl CodeFormatter for NoopCodeFormatter {
    fn format(&mut self, _reporter: &crate::Reporter, _files: &mut crate::GeneratedFiles) {}
}

// ---

mod cpp;
mod fbs;
mod python;
mod rust;

pub use self::{
    cpp::CppCodeFormatter, fbs::FbsCodeFormatter, python::PythonCodeFormatter,
    rust::RustCodeFormatter,
};
