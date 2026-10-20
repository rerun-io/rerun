#![cfg(feature = "testing")]
use crate::App;

pub trait AppTestingExt {
    fn testonly_set_recording_test_hook(&mut self, func: crate::app_state::TestHookRecordingFn);
    fn testonly_set_app_test_hook(&mut self, func: crate::app_state::TestHookAppFn);
}

impl AppTestingExt for App {
    fn testonly_set_recording_test_hook(&mut self, func: crate::app_state::TestHookRecordingFn) {
        self.state.test_hook_recording = Some(func);
    }

    fn testonly_set_app_test_hook(&mut self, func: crate::app_state::TestHookAppFn) {
        self.state.test_hook_app = Some(func);
    }
}
