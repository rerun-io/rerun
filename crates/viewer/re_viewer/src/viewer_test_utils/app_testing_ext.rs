#![cfg(feature = "testing")]
use re_ui::notifications::NotificationUi;
use re_viewer_context::{Route, StoreHub};

use crate::App;

pub trait AppTestingExt {
    fn testonly_get_store_hub(&mut self) -> &mut StoreHub;
    fn testonly_get_route(&self) -> &Route;
    fn testonly_set_recording_test_hook(&mut self, func: crate::app_state::TestHookRecordingFn);
    fn testonly_set_app_test_hook(&mut self, func: crate::app_state::TestHookAppFn);
    fn testonly_get_notifications(&self) -> &NotificationUi;
}

impl AppTestingExt for App {
    fn testonly_get_store_hub(&mut self) -> &mut StoreHub {
        self.store_hub
            .as_mut()
            .expect("store_hub should be initialized")
    }

    fn testonly_get_route(&self) -> &Route {
        self.state.navigation.current()
    }

    fn testonly_set_recording_test_hook(&mut self, func: crate::app_state::TestHookRecordingFn) {
        self.state.test_hook_recording = Some(func);
    }

    fn testonly_set_app_test_hook(&mut self, func: crate::app_state::TestHookAppFn) {
        self.state.test_hook_app = Some(func);
    }

    fn testonly_get_notifications(&self) -> &NotificationUi {
        &self.notifications
    }
}
