pub struct SuperluminalEndEventOnDrop;

impl Drop for SuperluminalEndEventOnDrop {
    fn drop(&mut self) {
        superluminal_perf::end_event();
    }
}
