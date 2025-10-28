use std::net::SocketAddr;

use chrono::format;
use pyo3::{
    Bound, PyResult, Python,
    exceptions::PyValueError,
    pyclass, pymethods,
    types::{
        PyAnyMethods as _, PyDict, PyDictMethods as _, PyModule, PyModuleMethods as _, PyString,
    },
};
use re_sdk::external::re_server::Args as ServerArgs;

/// Register the `rerun.server` module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyServerInternal>()?;

    Ok(())
}

#[pyclass(name = "ServerInternal", module = "rerun_bindings.rerun_bindings")] // NOLINT: skip pyclass_eq, non-trivial implementation
pub struct PyServerInternal {
    handle: re_sdk::external::re_server::ServerHandle,
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
    ) -> PyResult<(Self)> {
        let datasets = datasets
            .map(|d| {
                d.iter()
                    .map(|(k, v)| {
                        let name = k
                            .downcast::<PyString>()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_owned();
                        let path = v.extract::<&str>().unwrap();
                        format!("{name}={path}")
                    })
                    .filter_map(|d| d.parse::<re_sdk::external::re_server::NamedPath>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut args = ServerArgs {
            addr: address.unwrap_or("0.0.0.0".to_owned()),
            port: port.unwrap_or(51234),
            datasets,,
            tables: vec![],
        };

        let addr = SocketAddr::new(args.addr.parse().unwrap(), args.port);
        crate::utils::wait_for_future(py, async {
            let handle = match args.create_server_handle().await {
                Ok(handle) => {
                    re_log::info!("Started Rerun server on: {}", addr);
                    Some(handle)
                }
                Err(err) => {
                    re_log::error!("Failed to start Rerun server: {err:?}");
                    None
                }
            };

            if let Some(handle) = handle {
                Ok(Self {
                    handle,
                    address: addr,
                })
            } else {
                Err(PyValueError::new_err(format!(
                    "Failed to start Rerun server"
                )))
            }
        })
    }

    pub fn address(&self) -> PyResult<String> {
        Ok(format!("rerun+http://{}", self.address))
    }
}
