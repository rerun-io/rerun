use super::Index;

impl From<u32> for Index {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
