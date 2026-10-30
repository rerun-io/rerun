//! The viewer's own tools for a model-API agent: the viewer-control operations, run in-process.

use std::sync::Arc;

use futures::future::BoxFuture;
use re_agent::{AgentToolbox, ToolDefinition, ToolOutput};
use re_async::UiWaker;
use re_log_channel::{UiCallback, ViewerControlError};
use re_protos::viewer_control::v1alpha1::{
    ViewerControlRequest, ViewerControlResponse, viewer_control_response,
};
use re_viewer_control_tools::{ScreenshotDelivery, TOOL_NAME_PREFIX};
use serde_json::{Map, Value};

/// A model-API agent sees images directly. See [`ScreenshotDelivery::Inline`].
const SCREENSHOT_DELIVERY: ScreenshotDelivery = ScreenshotDelivery::Inline;

/// How many calls may wait for the UI thread on native. The agent runs its tool calls one at a
/// time, so more than one waiting means the viewer has stopped serving them.
#[cfg(not(target_arch = "wasm32"))]
const MAX_WAITING_CALLS: usize = 4;

/// Why a viewer-control tool call failed. The message is sent back to the model.
#[derive(Debug, thiserror::Error)]
enum ViewerToolError {
    #[error("The viewer is not answering tool calls right now")]
    Busy,

    #[error("The viewer dropped `{name}` without answering")]
    Dropped { name: String },

    #[error("`{name}` failed: {message}")]
    Failed { name: String, message: String },

    #[error("{message}")]
    InvalidRequest { message: String },

    #[error("The viewer is no longer running")]
    Stopped,

    #[error("The viewer did not answer `{name}` in time")]
    TimedOut { name: String },

    #[error("There is no tool named `{name}`")]
    UnknownTool { name: String },
}

/// What the viewer answers to one viewer-control operation.
type ViewerControlResult = Result<ViewerControlResponse, ViewerControlError>;

/// The channel that a [`ViewerToolbox`] queues its calls on, for the UI thread to run.
///
/// Unbounded on the web: there the agent and the UI share one thread, so a call cannot pile up
/// behind a stalled UI.
pub fn calls_channel() -> (
    crossbeam::channel::Sender<QueuedCall>,
    crossbeam::channel::Receiver<QueuedCall>,
) {
    cfg_select! {
        target_arch = "wasm32" => crossbeam::channel::unbounded(),
        _ => crossbeam::channel::bounded(MAX_WAITING_CALLS),
    }
}

/// One viewer-control operation the agent asked for, waiting for the UI thread to run it.
pub struct ViewerControlCall {
    /// What to do.
    pub request: ViewerControlRequest,

    /// Receives the result once the viewer has run the operation.
    pub on_done: UiCallback<ViewerControlResult>,
}

/// A call as it waits in the queue, before the UI thread takes it.
pub struct QueuedCall {
    request: ViewerControlRequest,
    reply_tx: futures::channel::oneshot::Sender<ViewerControlResult>,
}

impl QueuedCall {
    /// The call to run, or `None` if the agent stopped waiting for it.
    ///
    /// A call that timed out was reported to the agent as failed, so running it later (for
    /// example when a hidden tab renders again) would apply a change the agent believes did not
    /// happen.
    pub fn into_call(self) -> Option<ViewerControlCall> {
        let Self { request, reply_tx } = self;
        if reply_tx.is_canceled() {
            re_log::debug!("Dropping a viewer-control call that the agent stopped waiting for");
            return None;
        }
        Some(ViewerControlCall {
            request,
            on_done: UiCallback::new(move |result| {
                reply_tx.send(result).ok();
            }),
        })
    }
}

/// Offers the viewer-control operations as tools, and queues every call for the UI thread.
pub struct ViewerToolbox {
    calls_tx: crossbeam::channel::Sender<QueuedCall>,
    waker: UiWaker,

    /// Built once: every tool, as the service describes it.
    tools: Vec<re_viewer_control_tools::ToolDefinition>,
}

impl ViewerToolbox {
    /// Calls go to `calls_tx`. `waker` is woken for each, so the viewer runs it promptly.
    pub fn new(calls_tx: crossbeam::channel::Sender<QueuedCall>, waker: UiWaker) -> Arc<Self> {
        Arc::new(Self {
            calls_tx,
            waker,
            tools: re_viewer_control_tools::tools(SCREENSHOT_DELIVERY),
        })
    }

    /// Queues a call of the operation `operation`, and returns where its result will arrive.
    fn send(
        &self,
        operation: &str,
        arguments: Map<String, Value>,
    ) -> Result<futures::channel::oneshot::Receiver<ViewerControlResult>, ViewerToolError> {
        let request =
            re_viewer_control_tools::build_request(operation, Some(arguments), SCREENSHOT_DELIVERY)
                .map_err(|message| ViewerToolError::InvalidRequest { message })?;

        let (reply_tx, reply_rx) = futures::channel::oneshot::channel();
        let call = QueuedCall { request, reply_tx };
        self.calls_tx.try_send(call).map_err(|err| match err {
            crossbeam::channel::TrySendError::Full(_) => ViewerToolError::Busy,
            crossbeam::channel::TrySendError::Disconnected(_) => ViewerToolError::Stopped,
        })?;
        self.waker.wake();
        Ok(reply_rx)
    }
}

impl AgentToolbox for ViewerToolbox {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|tool| ToolDefinition {
                name: format!("{TOOL_NAME_PREFIX}{}", tool.name),
                description: tool.description.clone(),
                input_schema: tool.input_schema.clone(),
            })
            .collect()
    }

    fn requires_approval(&self, name: &str) -> bool {
        let operation = name.strip_prefix(TOOL_NAME_PREFIX);
        self.tools
            .iter()
            .find(|tool| Some(tool.name.as_str()) == operation)
            .is_none_or(|tool| tool.requires_approval)
    }

    fn call(
        &self,
        name: &str,
        arguments: Map<String, Value>,
    ) -> BoxFuture<'static, Result<Vec<ToolOutput>, Box<dyn std::error::Error + Send + Sync>>> {
        let Some(operation) = name.strip_prefix(TOOL_NAME_PREFIX).map(str::to_owned) else {
            let err = ViewerToolError::UnknownTool {
                name: name.to_owned(),
            };
            return Box::pin(std::future::ready(Err(err.into())));
        };
        let name = name.to_owned();
        let answer = self.send(&operation, arguments);
        Box::pin(async move {
            // The viewer answers from the UI thread, which may stop running (a minimized window,
            // a hidden tab), so the agent gets an error it can recover from instead of waiting.
            let deadline = std::pin::pin!(re_async::sleep(re_log_channel::VIEWER_CONTROL_TIMEOUT));
            let answer = match futures::future::select(answer?, deadline).await {
                futures::future::Either::Left((answer, _)) => answer,
                futures::future::Either::Right(((), _)) => {
                    return Err(ViewerToolError::TimedOut { name }.into());
                }
            };
            let response = answer
                .map_err(|_canceled| ViewerToolError::Dropped { name: name.clone() })?
                .map_err(|err| ViewerToolError::Failed {
                    name: name.clone(),
                    message: err.message,
                })?;
            if let Some(viewer_control_response::Kind::SaveScreenshot(screenshot)) = &response.kind
                && let Some(png) = &screenshot.png
            {
                return Ok(vec![ToolOutput::Image {
                    bytes: png.as_slice().into(),
                    mime_type: "image/png".to_owned(),
                }]);
            }
            let json = re_viewer_control_tools::response_json(&operation, &response)
                .map_err(|message| ViewerToolError::Failed { name, message })?;
            Ok(vec![ToolOutput::Text(json.to_string())])
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_the_agent_stopped_waiting_for_do_not_run() {
        let (calls_tx, calls_rx) = calls_channel();
        let toolbox = ViewerToolbox::new(calls_tx, UiWaker::noop());

        let _waiting = toolbox.send("get_state", Map::new());
        let timed_out = toolbox.send("get_state", Map::new());
        drop(timed_out);

        let calls = calls_rx
            .try_iter()
            .filter_map(QueuedCall::into_call)
            .count();
        assert_eq!(calls, 1);
    }

    #[test]
    fn screenshots_cannot_be_written_to_disk() {
        let (calls_tx, calls_rx) = calls_channel();
        let toolbox = ViewerToolbox::new(calls_tx, UiWaker::noop());

        let schema = toolbox
            .definitions()
            .into_iter()
            .find(|tool| tool.name == "rerun_save_screenshot")
            .expect("the screenshot tool")
            .input_schema;
        assert!(schema["properties"].get("file_path").is_none());

        let arguments = Map::from_iter([(
            "file_path".to_owned(),
            Value::String("/tmp/a.png".to_owned()),
        )]);
        assert!(toolbox.send("save_screenshot", arguments).is_err());
        assert!(
            calls_rx.try_recv().is_err(),
            "nothing must reach the viewer"
        );
    }
}
