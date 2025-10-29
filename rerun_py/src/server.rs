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

#[pyclass(name = "ServerInternal", module = "rerun_bindings.rerun_bindings")] // NOLINT: skip pyclass_eq, non-trivial implementation
pub struct PyServerInternal {
    handle: Option<re_server::ServerHandle>,
    address: SocketAddr,
}

#[pymethods]
impl PyServerInternal {
    #[new]
    #[pyo3(signature = (address=None, port=None, datasets=None))]
    pub fn new(
        py: Python<'_>,
        address: Option<String>,
        port: Option<u16>,
        datasets: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let datasets = datasets
            .map(|dict| {
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
            .unwrap_or_default();

        // we can re-use the CLI argument to construct the server
        let args = ServerArgs {
            addr: address.unwrap_or("0.0.0.0".to_owned()),
            port: port.unwrap_or(51234),
            datasets,
            tables: vec![],
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

    /// Get the server's connection address.
    ///
    /// Returns the address that clients can use to connect to this server,
    /// formatted as a `rerun+http://` URL.
    pub fn address(&self) -> String {
        format!("rerun+http://{}", self.address)
    }

    /// Shutdown the server, blocking until it has fully stopped.
    ///
    /// If the server is not running, raises a `ValueError`.
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

    /// Check if the server is currently running.
    pub fn is_running(&self) -> bool {
        self.handle.is_some()
    }
}

/// Register the `rerun.server` module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyServerInternal>()?;

    Ok(())
}
