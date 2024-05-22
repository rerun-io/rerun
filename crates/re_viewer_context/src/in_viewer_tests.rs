use std::any::Any;
use std::error::Error;

use ahash::HashMap;
use once_cell::sync::OnceCell;

use re_data_store::{StoreDiffKind, StoreEvent, StoreSubscriber, StoreSubscriberHandle};
use re_log_types::StoreId;
use re_types::external::arrow2;

use crate::{SystemCommand, SystemCommandSender, ViewerContext};

pub enum ExecuteOutcome {
    RunAgain,
    Success,
    Failure(Box<dyn Error>),
}

pub trait InViewerTest: Sync + Send {
    fn execute(&mut self, frame_nr: u64, ctx: &ViewerContext<'_>) -> ExecuteOutcome;
}

#[derive(Default)]
pub struct InViewerTestManager {
    tests: HashMap<&'static str, Box<dyn (Fn() -> Box<dyn InViewerTest>) + Sync + Send>>,
    running_tests: HashMap<StoreId, Vec<Box<dyn InViewerTest>>>,
}

impl InViewerTestManager {
    pub fn subscription_handle() -> StoreSubscriberHandle {
        static SUBSCRIPTION: OnceCell<re_data_store::StoreSubscriberHandle> = OnceCell::new();
        *SUBSCRIPTION
            .get_or_init(|| re_data_store::DataStore::register_subscriber(Box::<Self>::default()))
    }

    pub fn register_test<T: InViewerTest + Default + 'static>(trigger_name: &'static str) {
        re_data_store::DataStore::with_subscriber_mut(
            Self::subscription_handle(),
            |subscriber: &mut InViewerTestManager| {
                subscriber
                    .tests
                    .insert(trigger_name, Box::new(|| Box::new(T::default())));
            },
        );
    }

    pub fn run_tests(frame_nr: u64, ctx: &ViewerContext<'_>) {
        re_data_store::DataStore::with_subscriber_mut(
            Self::subscription_handle(),
            |subscriber: &mut InViewerTestManager| subscriber.run_tests_impl(frame_nr, ctx),
        );
    }

    fn run_tests_impl(&mut self, frame_nr: u64, ctx: &ViewerContext<'_>) {
        for (store_id, running_tests) in &mut self.running_tests {
            if !ctx.store_context.is_active(store_id) {
                continue;
            }

            running_tests.retain_mut(|test| {
                let result = test.execute(frame_nr, ctx);

                match result {
                    ExecuteOutcome::RunAgain => true,
                    ExecuteOutcome::Success => false,
                    ExecuteOutcome::Failure(err) => {
                        re_log::error!("{err}");
                        false
                    }
                }
            });

            if running_tests.is_empty() {
                ctx.command_sender
                    .send_system(SystemCommand::CloseStore(store_id.clone()));
            }
        }
    }
}

impl StoreSubscriber for InViewerTestManager {
    fn name(&self) -> String {
        "in_viewer_test_runner".to_owned()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn on_events(&mut self, events: &[StoreEvent]) {
        for event in events {
            match event.diff.kind {
                StoreDiffKind::Addition => {
                    if event.entity_path == "/test_trigger".into() {
                        for (component_name, cell) in &event.diff.cells {
                            if component_name == "TestTriggerIndicator" {
                                if let Some(test_name) = cell
                                    .as_arrow_ref()
                                    .as_any()
                                    .downcast_ref::<arrow2::array::Utf8Array<i32>>()
                                    .unwrap()
                                    .values_iter()
                                    .collect::<Vec<_>>()
                                    .first()
                                {
                                    if let Some(test_factory) = self.tests.get(test_name) {
                                        self.running_tests
                                            .entry(event.store_id.clone())
                                            .or_default()
                                            .push(test_factory());
                                    }
                                }
                            }
                        }
                    }
                }
                StoreDiffKind::Deletion => {}
            }
        }
    }
}
