use std::net::SocketAddr;

use pyo3::exceptions::PyValueError;
use pyo3::types::{
    PyAnyMethods as _, PyDict, PyDictMethods as _, PyModule, PyModuleMethods as _, PyString,
    PyStringMethods as _,
};
use pyo3::{Bound, PyResult, Python, pyclass, pymethods};
use re_server::{self, Args as ServerArgs, NamedPathCollection};

#[pyclass(name = "_ServerInternal", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq], non-trivial implementation
pub struct PyServerInternal {
    handle: Option<re_server::ServerHandle>,
    address: SocketAddr,
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyServerInternal {
    #[new]
    #[pyo3(signature = (*, address, port, datasets, dataset_prefixes, tables))]
    #[pyo3(text_signature = "(self, *, address, port, datasets, dataset_prefixes, tables)")]
    pub fn new(
        py: Python<'_>,
        address: &str,
        port: u16,
        datasets: &Bound<'_, PyDict>,
        dataset_prefixes: &Bound<'_, PyDict>,
        tables: &Bound<'_, PyDict>,
    ) -> PyResult<Self> {
        let datasets = extract_named_collections(datasets);
        let dataset_prefixes = extract_named_paths(dataset_prefixes);
        let tables = extract_named_paths(tables);

        // we can re-use the CLI argument to construct the server
        let args = ServerArgs {
            addr: address.to_owned(),
            port,
            datasets,
            dataset_prefixes,
            tables,
        };

        let address = SocketAddr::new(
            args.addr.parse().map_err(|err| {
                PyValueError::new_err(format!("Invalid address: {}: {err}", args.addr))
            })?,
            args.port,
        );

        crate::utils::wait_for_future(py, async {
            let (handle, _) = args.create_server_handle().await.map_err(|err| {
                PyValueError::new_err(format!("Failed to start Rerun server: {err:#}"))
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

fn extract_named_paths(dict: &Bound<'_, PyDict>) -> Vec<re_server::NamedPath> {
    dict.iter()
        .filter_map(|(k, v)| {
            let name = k.downcast::<PyString>().ok()?;
            let path = v.extract::<&str>().ok()?;

            Some(re_server::NamedPath {
                name: Some(name.to_string_lossy().to_string()),
                path: std::path::PathBuf::from(path),
            })
        })
        .collect()
}

fn extract_named_collections(dict: &Bound<'_, PyDict>) -> Vec<NamedPathCollection> {
    dict.iter()
        .filter_map(|(k, v)| {
            let name = k.downcast::<PyString>().ok()?;
            let paths: Vec<String> = v.extract().ok()?;

            Some(NamedPathCollection {
                name: name.to_string_lossy().to_string(),
                paths: paths.into_iter().map(std::path::PathBuf::from).collect(),
            })
        })
        .collect()
}

/// Register the `rerun.server` module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyServerInternal>()?;

    Ok(())
}
