//! Python bindings for the `ViewerControlService` API, defined by `viewer_control.proto`.
//! That file lists every place an operation has to be added.

#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

use arrow::array::RecordBatch;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};
use re_grpc_client::write_table::channel;
use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;
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

    /// Send any viewer-control operation, as a `ViewerControlRequest` in canonical protobuf JSON,
    /// and return the `ViewerControlResponse` in the same form.
    ///
    /// Blocks until the viewer answers, for at most `re_log_channel::VIEWER_CONTROL_TIMEOUT`
    /// (15 seconds), after which it fails.
    fn viewer_control(self_: Py<Self>, request_json: &str, py: Python<'_>) -> PyResult<String> {
        let mut conn = self_.borrow(py).conn.clone();

        conn.viewer_control(py, request_json)
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

    fn viewer_control(&mut self, py: Python<'_>, request_json: &str) -> PyResult<String> {
        let request: re_protos::viewer_control::v1alpha1::ViewerControlRequest =
            re_protos::json::from_json_str(request_json)
                .map_err(|err| PyValueError::new_err(err.to_string()))?;

        let response =
            wait_for_future(py, self.control_client.viewer_control(request)).map_err(to_py_err)?;

        to_json(&response.into_inner())
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
