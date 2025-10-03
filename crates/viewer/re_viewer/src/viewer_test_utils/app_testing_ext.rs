use re_viewer_context::StoreHub;
use re_viewer_context::ViewerContext;

use crate::App;

pub trait AppTestingExt {
    fn testonly_get_store_hub(&mut self) -> &mut StoreHub;
    fn testonly_set_test_hook(&mut self, func: Option<Box<dyn FnOnce(&ViewerContext<'_>)>>);
}

impl AppTestingExt for App {
    fn testonly_get_store_hub(&mut self) -> &mut StoreHub {
        self.store_hub
            .as_mut()
            .expect("store_hub should be initialized")
    }

    fn testonly_set_test_hook(&mut self, func: Option<Box<dyn FnOnce(&ViewerContext<'_>)>>) {
        self.state.test_hook = func;
    }
}
