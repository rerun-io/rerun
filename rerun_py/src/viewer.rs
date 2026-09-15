//! Python bindings for the `ViewerControlService` API, defined by `viewer_control.proto`.
//! That file lists every place an operation has to be added.

#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

use arrow::array::RecordBatch;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};
use re_grpc_client::write_table::channel;
use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;
use re_protos::viewer_control::v1alpha1::ViewerControlOp as _;
use re_protos::viewer_control::v1alpha1::viewer_control_service_client::ViewerControlServiceClient;

use crate::catalog::to_py_err;
use crate::utils::wait_for_future;

/// Register the `rerun.catalog` module.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyViewerClientInternal>()?;

    Ok(())
}

/// A connection to an instance of a Rerun viewer.
#[pyclass(
    name = "ViewerClientInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyViewerClientInternal {
    conn: ViewerConnectionHandle,
}
#[pymethods]
impl PyViewerClientInternal {
    #[new]
    #[pyo3(text_signature = "(self, addr)")]
    fn new(py: Python<'_>, addr: &str) -> PyResult<Self> {
        let origin = addr.parse::<re_uri::Origin>().map_err(to_py_err)?;

        let conn = ViewerConnectionHandle::new(py, origin.clone())?;

        Ok(Self { conn })
    }

    fn send_table(
        self_: Py<Self>,
        id: String,
        table: arrow::pyarrow::PyArrowType<RecordBatch>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let mut conn = self_.borrow(py).conn.clone();

        conn.send_table(py, id, table)
    }

    #[pyo3(signature = (target = None, store_ids = None))]
    fn close_recordings(
        self_: Py<Self>,
        target: Option<&str>,
        store_ids: Option<Vec<String>>,
        py: Python<'_>,
    ) -> PyResult<String> {
        let mut conn = self_.borrow(py).conn.clone();

        conn.close_recordings(py, target, store_ids)
    }

    fn open_url(self_: Py<Self>, url: String, py: Python<'_>) -> PyResult<()> {
        let mut conn = self_.borrow(py).conn.clone();

        conn.open_url(py, url)
    }

    fn save_screenshot(
        self_: Py<Self>,
        file_path: String,
        view_id: Option<Bound<'_, pyo3::PyAny>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let mut conn = self_.borrow(py).conn.clone();

        let view_id_str = view_id
            .map(|v| {
                v.extract::<String>()
                    .or_else(|_| v.str()?.extract::<String>())
            })
            .transpose()?;

        conn.save_screenshot(py, file_path, view_id_str)
    }

    #[pyo3(signature = (timeline, time, play, store_id = None))]
    fn set_time_cursor(
        self_: Py<Self>,
        timeline: Option<String>,
        time: i64,
        play: bool,
        store_id: Option<String>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let mut conn = self_.borrow(py).conn.clone();

        conn.set_time_cursor(py, timeline, time, play, store_id)
    }

    fn viewer_logs(
        self_: Py<Self>,
        after_sequence: Option<u64>,
        py: Python<'_>,
    ) -> PyResult<String> {
        let mut conn = self_.borrow(py).conn.clone();

        conn.viewer_logs(py, after_sequence)
    }

    fn viewer_state(self_: Py<Self>, py: Python<'_>) -> PyResult<String> {
        let mut conn = self_.borrow(py).conn.clone();

        conn.viewer_state(py)
    }
}

/// Connection handle for sending data to and controlling a viewer.
// TODO(grtlr): In the future, we probably want to merge this with the other APIs.
#[derive(Clone)]
pub struct ViewerConnectionHandle {
    client: MessageProxyServiceClient<tonic::transport::Channel>,
    control_client: ViewerControlServiceClient<tonic::transport::Channel>,
}

impl ViewerConnectionHandle {
    pub fn new(py: Python<'_>, origin: re_uri::Origin) -> PyResult<Self> {
        let channel = wait_for_future(py, channel(origin.clone())).map_err(to_py_err)?;

        let client = MessageProxyServiceClient::new(channel.clone())
            .max_decoding_message_size(re_grpc_client::MAX_DECODING_MESSAGE_SIZE);
        let control_client = ViewerControlServiceClient::new(channel)
            .max_decoding_message_size(re_grpc_client::MAX_DECODING_MESSAGE_SIZE);

        Ok(Self {
            client,
            control_client,
        })
    }
}

impl ViewerConnectionHandle {
    fn send_table(
        &mut self,
        py: Python<'_>,
        id: String,
        table: arrow::pyarrow::PyArrowType<RecordBatch>,
    ) -> PyResult<()> {
        wait_for_future(
            py,
            self.client
                .write_table(re_protos::sdk_comms::v1alpha1::WriteTableRequest {
                    id: Some(re_protos::common::v1alpha1::TableId { id }),
                    data: Some(table.0.into()),
                }),
        )
        .map_err(to_py_err)?;

        Ok(())
    }

    fn close_recordings(
        &mut self,
        py: Python<'_>,
        target: Option<&str>,
        store_ids: Option<Vec<String>>,
    ) -> PyResult<String> {
        use re_protos::viewer_control::v1alpha1::{
            ViewerRecordingIds, close_recordings_request::Target,
        };

        let target = match (target, store_ids) {
            (Some(target), None) => match target {
                "current" => Target::Current(true),
                "all" => Target::All(true),
                other => {
                    return Err(PyRuntimeError::new_err(format!(
                        "unknown close target {other:?}, expected \"current\" or \"all\""
                    )));
                }
            },

            (None, Some(store_ids)) => Target::StoreIds(ViewerRecordingIds { store_ids }),

            (None, None) | (Some(_), Some(_)) => {
                return Err(PyRuntimeError::new_err(
                    "pass exactly one of `target` or `store_ids`",
                ));
            }
        };
        let response = self.execute(
            py,
            re_protos::viewer_control::v1alpha1::CloseRecordingsRequest {
                target: Some(target),
            },
        )?;

        to_json(&response)
    }

    fn open_url(&mut self, py: Python<'_>, url: String) -> PyResult<()> {
        self.execute::<re_protos::viewer_control::v1alpha1::OpenUrlRequest>(
            py,
            re_protos::viewer_control::v1alpha1::OpenUrlRequest { url },
        )?;

        Ok(())
    }

    fn save_screenshot(
        &mut self,
        py: Python<'_>,
        file_path: String,
        view_id: Option<String>,
    ) -> PyResult<()> {
        let response = wait_for_future(
            py,
            self.control_client.viewer_control(
                re_protos::viewer_control::v1alpha1::SaveScreenshotRequest { view_id, file_path }
                    .into_envelope(),
            ),
        )
        .map_err(to_py_err)?;

        re_protos::viewer_control::v1alpha1::SaveScreenshotRequest::from_envelope(
            response.into_inner(),
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        Ok(())
    }

    fn set_time_cursor(
        &mut self,
        py: Python<'_>,
        timeline: Option<String>,
        time: i64,
        play: bool,
        store_id: Option<String>,
    ) -> PyResult<()> {
        let response = wait_for_future(
            py,
            self.control_client.viewer_control(
                re_protos::viewer_control::v1alpha1::SetTimeCursorRequest {
                    store_id,
                    timeline: timeline.map(|name| re_protos::common::v1alpha1::Timeline { name }),
                    time: Some(time.into()),
                    play: Some(play),
                }
                .into_envelope(),
            ),
        )
        .map_err(to_py_err)?;

        re_protos::viewer_control::v1alpha1::SetTimeCursorRequest::from_envelope(
            response.into_inner(),
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        Ok(())
    }

    fn viewer_logs(&mut self, py: Python<'_>, after_sequence: Option<u64>) -> PyResult<String> {
        let response = self.execute(
            py,
            re_protos::viewer_control::v1alpha1::GetViewerLogsRequest { after_sequence },
        )?;

        to_json(&response)
    }

    fn viewer_state(&mut self, py: Python<'_>) -> PyResult<String> {
        let response = self.execute(
            py,
            re_protos::viewer_control::v1alpha1::GetViewerStateRequest {},
        )?;

        to_json(&response)
    }

    /// Send one viewer-control operation and unwrap the matching response.
    ///
    /// Every operation goes through here, so the response variant is checked in one place rather
    /// than in each method.
    fn execute<Op>(&mut self, py: Python<'_>, request: Op) -> PyResult<Op::Response>
    where
        Op: re_protos::viewer_control::v1alpha1::ViewerControlOp,
    {
        let response = wait_for_future(
            py,
            self.control_client.viewer_control(request.into_envelope()),
        )
        .map_err(to_py_err)?;

        Op::from_envelope(response.into_inner())
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    }
}

/// Render a viewer-control response as canonical protobuf JSON, for the Python layer to parse.
///
/// This keeps `protobuf` out of the wheel's dependencies: the generated types stay on the Rust
/// side, and Python turns the JSON into its own dataclasses.
fn to_json<M>(message: &M) -> PyResult<String>
where
    M: re_protos::external::prost::Message + re_protos::external::prost::Name,
{
    re_protos::json::to_json_value(message)
        .map(|value| value.to_string())
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))
}
