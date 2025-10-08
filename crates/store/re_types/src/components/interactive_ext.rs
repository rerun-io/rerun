use super::Interactive;

impl Default for Interactive {
    #[inline]
    fn default() -> Self {
        Interactive(true.into())
    }
}
