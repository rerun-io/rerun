/// Implements the formatting pass.
pub trait CodeFormatter {
    /// Formats generated files in-place.
    fn format(&mut self, reporter: &crate::Reporter, files: &mut crate::GeneratedFiles);
}

// ---

mod cpp;
mod python;
mod rust;

pub use self::cpp::CppCodeFormatter;
pub use self::python::PythonCodeFormatter;
pub use self::rust::RustCodeFormatter;
