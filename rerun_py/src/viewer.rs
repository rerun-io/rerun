#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

use arrow::array::RecordBatch;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};
use re_grpc_client::write_table::viewer_client;
use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;

use crate::catalog::to_py_err;
use crate::utils::wait_for_future;

/// Register the `rerun.catalog` module.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyViewerClientInternal>()?;

    Ok(())
}

/// A connection to an instance of a Rerun viewer.
#[pyclass( // NOLINT: ignore[py-cls-eq] internal object
    name = "ViewerClientInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyViewerClientInternal {
    conn: ViewerConnectionHandle,
}
#[pymethods] // NOLINT: ignore[py-mthd-str]
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
}

/// Connection handle to the message proxy service.
///
/// This handle is modelled after [`crate::catalog::ConnectionHandle`] and only concerned with
/// table-related operations, most importantly `WriteTable`.
// TODO(grtlr): In the future, we probably want to merge this with the other APIs.
#[derive(Clone)]
pub struct ViewerConnectionHandle {
    client: MessageProxyServiceClient<tonic::transport::Channel>,
}

impl ViewerConnectionHandle {
    pub fn new(py: Python<'_>, origin: re_uri::Origin) -> PyResult<Self> {
        let client = wait_for_future(py, viewer_client(origin.clone())).map_err(to_py_err)?;

        Ok(Self { client })
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

    fn save_screenshot(
        &mut self,
        py: Python<'_>,
        file_path: String,
        view_id: Option<String>,
    ) -> PyResult<()> {
        wait_for_future(
            py,
            self.client
                .save_screenshot(re_protos::sdk_comms::v1alpha1::SaveScreenshotRequest {
                    view_id,
                    file_path,
                }),
        )
        .map_err(to_py_err)?;

        Ok(())
    }
}
