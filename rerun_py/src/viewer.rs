#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

use arrow::array::RecordBatch;
use pyo3::{prelude::*, Bound, PyResult};

use re_grpc_client::message_proxy::write_table::table_client;
use re_log_encoding::codec::wire::encoder::Encode as _;
use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;

use crate::{catalog::to_py_err, utils::wait_for_future};

/// Register the `rerun.catalog` module.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyViewerClient>()?;

    Ok(())
}

/// A connection to an instance of a Rerun viewer.
#[pyclass(name = "ViewerClient")]
pub struct PyViewerClient {
    conn: TableConnectionHandle,
}
#[pymethods]
impl PyViewerClient {
    /// Create a new viewer client object.
    #[new]
    fn new(py: Python<'_>, addr: String) -> PyResult<Self> {
        let origin = addr.as_str().parse::<re_uri::Origin>().map_err(to_py_err)?;

        let conn = TableConnectionHandle::new(py, origin.clone())?;

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
}

/// Connection handle to the message proxy service.
///
/// This handle is modelled after [`crate::ConnectionHandle`] and only concerned with
/// table-related operations, most importantly `WriteTable`.
// TODO(grtlr): In the future, we probably want to merge this with the other APIs.
#[derive(Clone)]
pub struct TableConnectionHandle {
    client: MessageProxyServiceClient<tonic::transport::Channel>,
}

impl TableConnectionHandle {
    pub fn new(py: Python<'_>, origin: re_uri::Origin) -> PyResult<Self> {
        let client = wait_for_future(py, table_client(origin.clone())).map_err(to_py_err)?;

        Ok(Self { client })
    }
}

impl TableConnectionHandle {
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
                    data: Some(table.0.encode().map_err(to_py_err)?),
                }),
        )
        .map_err(to_py_err)?;

        Ok(())
    }
}
