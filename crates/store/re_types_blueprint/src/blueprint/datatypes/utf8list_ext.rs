use super::Utf8List;

impl Utf8List {
    /// Iterates through the list of strings as Rust `str` references.
    pub fn iter(&self) -> impl Iterator<Item = &'_ str> {
        self.0.iter().map(|s| s.as_str())
    }
}
