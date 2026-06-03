use egui::emath;

use crate::SizeBytes;

impl<T: SizeBytes + Copy> SizeBytes for emath::History<T> {
    fn heap_size_bytes(&self) -> u64 {
        let s = std::mem::size_of::<(f64, T)>() as u64 * self.len() as u64;
        if T::IS_POD {
            s
        } else {
            s + self
                .iter()
                .map(|t: (f64, T)| t.heap_size_bytes())
                .sum::<u64>()
        }
    }
}
