use std::net::SocketAddr;

use pyo3::{
    Bound, PyResult, Python,
    exceptions::PyValueError,
    pyclass, pymethods,
    types::{
        PyAnyMethods as _, PyDict, PyDictMethods as _, PyModule, PyModuleMethods as _, PyString,
        PyStringMethods as _,
    },
};
use re_server::{self, Args as ServerArgs};

#[pyclass(name = "ServerInternal", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq], non-trivial implementation
pub struct PyServerInternal {
    handle: Option<re_server::ServerHandle>,
    address: SocketAddr,
}

#[pymethods]
impl PyServerInternal {
    #[new]
    #[pyo3(signature = (*, address="0.0.0.0", port=51234, datasets=None, tables=None))]
    pub fn new(
        py: Python<'_>,
        address: &str,
        port: u16,
        datasets: Option<&Bound<'_, PyDict>>,
        tables: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let datasets = extract_named_paths(datasets);
        let tables = extract_named_paths(tables);

        // we can re-use the CLI argument to construct the server
        let args = ServerArgs {
            addr: address.to_owned(),
            port,
            datasets,
            tables,
        };

        let address = SocketAddr::new(
            args.addr.parse().map_err(|err| {
                PyValueError::new_err(format!("Invalid address: {}: {err}", args.addr))
            })?,
            args.port,
        );

        crate::utils::wait_for_future(py, async {
            let handle = args.create_server_handle().await.map_err(|err| {
                PyValueError::new_err(format!("Failed to start Rerun server: {err}"))
            })?;

            Ok(Self {
                handle: Some(handle),
                address,
            })
        })
    }

    pub fn address(&self) -> String {
        format!("rerun+http://{}", self.address)
    }

    pub fn shutdown(&mut self, py: Python<'_>) -> PyResult<()> {
        if let Some(handle) = self.handle.take() {
            crate::utils::wait_for_future(py, async {
                handle.shutdown_and_wait().await;
                Ok(())
            })
        } else {
            Err(PyValueError::new_err(
                "Server is not running or has already been shut down",
            ))
        }
    }

    pub fn is_running(&self) -> bool {
        self.handle.is_some()
    }
}

fn extract_named_paths(dict: Option<&Bound<'_, PyDict>>) -> Vec<re_server::NamedPath> {
    dict.map(|dict| {
        dict.iter()
            .filter_map(|(k, v)| {
                let name = k.downcast::<PyString>().ok()?;
                let path = v.extract::<&str>().ok()?;

                Some(re_server::NamedPath {
                    name: Some(name.to_string_lossy().to_string()),
                    path: std::path::PathBuf::from(path),
                })
            })
            .collect::<Vec<_>>()
    })
    .unwrap_or_default()
}

/// Register the `rerun.server` module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyServerInternal>()?;

    Ok(())
}
