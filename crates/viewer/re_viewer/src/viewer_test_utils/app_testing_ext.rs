use re_viewer_context::StoreHub;

use crate::App;
use crate::app_state::TestHookFn;

pub trait AppTestingExt {
    fn testonly_get_store_hub(&mut self) -> &mut StoreHub;
    fn testonly_set_test_hook(&mut self, func: TestHookFn);
}

impl AppTestingExt for App {
    fn testonly_get_store_hub(&mut self) -> &mut StoreHub {
        self.store_hub
            .as_mut()
            .expect("store_hub should be initialized")
    }

    fn testonly_set_test_hook(&mut self, func: TestHookFn) {
        self.state.test_hook = Some(func);
    }
}
