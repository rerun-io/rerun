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
    host: std::net::IpAddr,
    url: String,
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyServerInternal {
    #[new]
    #[pyo3(signature = (*, host, port, datasets, dataset_prefixes, tables))]
    #[pyo3(text_signature = "(self, *, host, port, datasets, dataset_prefixes, tables)")]
    pub fn new(
        py: Python<'_>,
        host: &str,
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
            host: host.to_owned(),
            port,
            datasets,
            dataset_prefixes,
            tables,
            latency_ms: 0, // no artificial latency
        };

        let host: std::net::IpAddr = host
            .parse()
            .map_err(|err| PyValueError::new_err(format!("Invalid IP: {host:?}: {err}")))?;

        let connect_ip = if host.is_unspecified() {
            // We usually cannot connect to 0.0.0.0 or ::, so tell clients to connect to 127.0.0.1 instead:
            std::net::Ipv4Addr::LOCALHOST.into()
        } else {
            host
        };
        let connect_address = SocketAddr::new(connect_ip, args.port);

        let url = format!("rerun+http://{connect_address}");

        crate::utils::wait_for_future(py, async {
            let (handle, _) = args.create_server_handle().await.map_err(|err| {
                PyValueError::new_err(format!("Failed to start Rerun server: {err:#}"))
            })?;

            Ok(Self {
                handle: Some(handle),
                host,
                url,
            })
        })
    }

    /// The address of the server to which clients can connect.
    pub fn url(&self) -> String {
        self.url.clone()
    }

    /// Get the IP that we've bound the server to.
    pub fn host(&self) -> String {
        self.host.to_string()
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
