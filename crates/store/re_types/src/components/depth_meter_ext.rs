use super::DepthMeter;

impl Default for DepthMeter {
    #[inline]
    fn default() -> Self {
        DepthMeter(1.0.into()) // 1 unit == 1 meter.
    }
}
