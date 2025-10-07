#![cfg(feature = "testing")]
use re_viewer_context::StoreHub;

use crate::App;

pub trait AppTestingExt {
    fn testonly_get_store_hub(&mut self) -> &mut StoreHub;
    fn testonly_set_test_hook(&mut self, func: crate::app_state::TestHookFn);
}

impl AppTestingExt for App {
    fn testonly_get_store_hub(&mut self) -> &mut StoreHub {
        self.store_hub
            .as_mut()
            .expect("store_hub should be initialized")
    }

    fn testonly_set_test_hook(&mut self, func: crate::app_state::TestHookFn) {
        self.state.test_hook = Some(func);
    }
}
