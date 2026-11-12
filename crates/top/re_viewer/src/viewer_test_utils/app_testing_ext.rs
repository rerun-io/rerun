#![cfg(feature = "testing")]
use crate::App;

pub trait AppTestingExt {
    fn testonly_set_recording_test_hook(&mut self, func: crate::app::TestHookRecordingFn);
    fn testonly_set_app_test_hook(&mut self, func: crate::app::TestHookAppFn);
}

impl AppTestingExt for App {
    fn testonly_set_recording_test_hook(&mut self, func: crate::app::TestHookRecordingFn) {
        self.test_hooks.recording = Some(func);
    }

    fn testonly_set_app_test_hook(&mut self, func: crate::app::TestHookAppFn) {
        self.test_hooks.app = Some(func);
    }
}
