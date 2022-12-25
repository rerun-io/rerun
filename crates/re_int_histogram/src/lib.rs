pub mod bad;
pub mod better;
pub mod binary;

// pub use binary_idea::IntHistogram;

/// Baseline for performance and memory
#[derive(Default)]
pub struct BTreeeIntHistogram {
    map: std::collections::BTreeMap<i64, u32>,
}
impl BTreeeIntHistogram {
    pub fn increment(&mut self, key: i64, inc: u32) {
        *self.map.entry(key).or_default() += inc;
    }
}
