use ahash::HashMap;
use egui::{Context, Id, Ui};

type CleanupClosure = Box<dyn FnOnce(&Context) + Send + Sync + 'static>;

#[derive(Default)]
pub struct EguiMemoryGarbageCollector {
    /// Anything stored here will move to `pending_cleanup` at end of frame.
    seen_this_frame: HashMap<Id, CleanupClosure>,

    /// Anything still in here was not shown this frame and will be cleared.
    pending_cleanup: HashMap<Id, CleanupClosure>,
}

impl EguiMemoryGarbageCollector {
    pub fn add(&mut self, id: Id, cleanup: impl FnOnce(&Context) + Send + Sync + 'static) {
        self.seen_this_frame.insert(id, Box::new(cleanup));
        self.pending_cleanup.remove(&id);
    }
}

impl egui::Plugin for EguiMemoryGarbageCollector {
    fn debug_name(&self) -> &'static str {
        "GarbageCollector"
    }

    fn on_end_pass(&mut self, ui: &mut Ui) {
        let cleanup = std::mem::take(&mut self.pending_cleanup);
        std::mem::swap(&mut self.seen_this_frame, &mut self.pending_cleanup);
        #[expect(clippy::iter_over_hash_type)]
        // The cleanups are independent, so order does not matter.
        for (_, clean) in cleanup {
            clean(ui.ctx());
        }
    }
}
