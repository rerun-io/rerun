use std::rc::Rc;

use re_viewer_context::ItemCollection;

pub struct Callbacks {
    pub on_selection_change: Rc<dyn Fn(&ItemCollection)>,
    // TODO(jan, andreas): why not add all the stuff from TimelineCallbacks here as well? ;-)
}

impl Callbacks {
    pub fn on_selection_change(&self, items: &ItemCollection) {
        (self.on_selection_change)(items);
    }
}
